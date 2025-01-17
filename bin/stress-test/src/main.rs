use std::{
    path::{Path, PathBuf},
    sync::mpsc::channel,
    time::{Duration, Instant},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use miden_lib::{
    accounts::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    transaction::TransactionKernel,
};
use miden_node_block_producer::{
    batch_builder::TransactionBatch, block_builder::BlockBuilder, store::StoreClient,
    test_utils::MockProvenTxBuilder,
};
use miden_node_proto::generated::store::api_client::ApiClient;
use miden_node_store::{config::StoreConfig, server::Store};
use miden_objects::{
    accounts::{AccountBuilder, AccountIdAnchor, AccountStorageMode, AccountType},
    assets::{Asset, FungibleAsset, TokenSymbol},
    crypto::dsa::rpo_falcon512::SecretKey,
    testing::notes::NoteBuilder,
    transaction::OutputNote,
    Digest, Felt,
};
use miden_processor::crypto::RpoRandomCoin;
use rand::Rng;
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    prelude::*,
};
use tokio::task;

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    SeedStore {
        #[arg(short, long, value_name = "DUMP_FILE", default_value = "./miden-store.sqlite3")]
        dump_file: PathBuf,

        #[arg(short, long, value_name = "ACCOUNTS_NUMBER")]
        accounts_number: usize,

        #[arg(short, long, value_name = "GENESIS_FILE")]
        genesis_file: PathBuf,
    },
}

const BATCHES_PER_BLOCK: usize = 16;
const TRANSACTIONS_PER_BATCH: usize = 16;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Command::SeedStore { dump_file, accounts_number, genesis_file } => {
            seed_store(dump_file, *accounts_number, genesis_file).await;
        },
    }
}

async fn seed_store(dump_file: &Path, accounts_number: usize, genesis_file: &Path) {
    let store_config = StoreConfig {
        database_filepath: dump_file.to_path_buf(),
        genesis_filepath: genesis_file.to_path_buf(),
        ..Default::default()
    };

    // Start store
    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });

    // Start block builder
    let store_client =
        StoreClient::new(ApiClient::connect(store_config.endpoint.to_string()).await.unwrap());
    let block_builder = BlockBuilder::new(store_client);

    println!("Creating new faucet account...");
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    let key_pair = SecretKey::with_rng(&mut rng);
    let init_seed = [0_u8; 32];
    let (new_faucet, _seed) = AccountBuilder::new(init_seed)
        .anchor(AccountIdAnchor::PRE_GENESIS)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Private)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(
            BasicFungibleFaucet::new(TokenSymbol::new("TEST").unwrap(), 2, Felt::new(100_000))
                .unwrap(),
        )
        .build()
        .unwrap();
    let faucet_id = new_faucet.id();

    let (batch_sender, batch_receiver) = channel::<TransactionBatch>();

    println!("Inserting blocks...");
    let load_start = Instant::now();

    // Spawn a task for block building
    let db_task = task::spawn(async move {
        let mut current_block: Vec<TransactionBatch> = Vec::with_capacity(BATCHES_PER_BLOCK);
        let mut insertion_times = Vec::new();
        while let Ok(batch) = batch_receiver.recv() {
            current_block.push(batch);

            if current_block.len() == BATCHES_PER_BLOCK {
                let start = Instant::now();
                block_builder.build_block(&current_block).await.unwrap();
                insertion_times.push(start.elapsed());
                current_block.clear();
            }
        }

        if !current_block.is_empty() {
            let start = Instant::now();
            block_builder.build_block(&current_block).await.unwrap();
            insertion_times.push(start.elapsed());
        }

        // Print insertion times
        let insertions = insertion_times.len() as u32;
        println!("Inserted {} blocks", insertions);

        if insertions == 0 {
            return;
        }

        let total_time: Duration = insertion_times.iter().sum();
        let avg_time = total_time / insertions;
        println!("Average insertion time: {} ms", avg_time.as_millis());
    });

    // Parallel account grinding and batch generation
    (0..accounts_number)
        .into_par_iter()
        .map(|_| {
            let (new_account, _) = AccountBuilder::new(init_seed)
                .anchor(AccountIdAnchor::PRE_GENESIS)
                .account_type(AccountType::RegularAccountImmutableCode)
                .storage_mode(AccountStorageMode::Private)
                .with_component(RpoFalcon512::new(key_pair.public_key()))
                .with_component(BasicWallet)
                .build()
                .unwrap();
            new_account.id()
        })
        .map(|account_id| {
            let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
            let coin_seed: [u64; 4] = rand::thread_rng().gen();
            let rng: RpoRandomCoin = RpoRandomCoin::new(coin_seed.map(Felt::new));
            let note = NoteBuilder::new(faucet_id, rng)
                .add_assets(vec![asset])
                .build(&TransactionKernel::assembler())
                .unwrap();

            let create_notes_tx =
                MockProvenTxBuilder::with_account(faucet_id, Digest::default(), Digest::default())
                    .output_notes(vec![OutputNote::Full(note.clone())])
                    .build();

            let consume_notes_txs =
                MockProvenTxBuilder::with_account(account_id, Digest::default(), Digest::default())
                    .unauthenticated_notes(vec![note])
                    .build();
            [create_notes_tx, consume_notes_txs]
        })
        .chunks(TRANSACTIONS_PER_BATCH / 2)
        .for_each_with(batch_sender.clone(), |sender, txs| {
            let batch =
                TransactionBatch::new(txs.concat().iter().collect::<Vec<_>>(), Default::default())
                    .unwrap();
            sender.send(batch).unwrap()
        });
    drop(batch_sender);

    db_task.await.unwrap();
    println!("Store loaded in {:?} seconds", load_start.elapsed().as_secs());
}
