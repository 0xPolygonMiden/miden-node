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
    accounts::{AccountBuilder, AccountId, AccountStorageMode, AccountType},
    assets::{Asset, FungibleAsset, TokenSymbol},
    crypto::dsa::rpo_falcon512::SecretKey,
    testing::notes::NoteBuilder,
    transaction::OutputNote,
    BlockHeader, Digest, Felt,
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

        #[arg(short, long, value_name = "NUM_ACCOUNTS")]
        num_accounts: usize,

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
        Command::SeedStore { dump_file, num_accounts, genesis_file } => {
            seed_store(dump_file, *num_accounts, genesis_file).await;
        },
    }
}

async fn create_faucet(anchor_block: &BlockHeader) -> AccountId {
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    let key_pair = SecretKey::with_rng(&mut rng);
    let init_seed = [0_u8; 32];

    let (new_faucet, _seed) = AccountBuilder::new(init_seed)
        .anchor(anchor_block.try_into().unwrap())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Private)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(
            BasicFungibleFaucet::new(TokenSymbol::new("TEST").unwrap(), 2, Felt::new(100_000))
                .unwrap(),
        )
        .build()
        .unwrap();
    new_faucet.id()
}

async fn seed_store(dump_file: &Path, num_accounts: usize, genesis_file: &Path) {
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

    let genesis_header = store_client.latest_header().await.unwrap();

    let block_builder = BlockBuilder::new(store_client);

    println!("Creating new faucet account...");
    let faucet_id = create_faucet(&genesis_header).await;

    let (batch_sender, batch_receiver) = channel::<TransactionBatch>();
    let (msg_sender, msg_receiver) = channel::<u8>();

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
                msg_sender.send(1).unwrap();
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

    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    // Re-uing the same key for all accounts to avoid Falcon key generation overhead
    let key_pair = SecretKey::with_rng(&mut rng);

    // Create notes and build blocks
    let mut notes = vec![];
    let mut create_note_txs = vec![];
    println!("Creating notes...");
    for _ in 0..num_accounts {
        let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
        let coin_seed: [u64; 4] = rand::thread_rng().gen();
        let rng: RpoRandomCoin = RpoRandomCoin::new(coin_seed.map(Felt::new));
        let note = NoteBuilder::new(faucet_id, rng)
            .add_assets(vec![asset])
            .build(&TransactionKernel::assembler())
            .unwrap();

        notes.push(note.clone());

        let create_notes_tx =
            MockProvenTxBuilder::with_account(faucet_id, Digest::default(), Digest::default())
                .output_notes(vec![OutputNote::Full(note)])
                .build();

        create_note_txs.push(create_notes_tx);
    }

    for txs in create_note_txs.chunks(TRANSACTIONS_PER_BATCH) {
        let batch =
            TransactionBatch::new(txs.iter().collect::<Vec<_>>(), Default::default()).unwrap();
        batch_sender.send(batch).unwrap()
    }

    // TODO: use BlockNoteTree to get inclusion proofs of all notes
    let store_client =
        StoreClient::new(ApiClient::connect(store_config.endpoint.to_string()).await.unwrap());

    msg_receiver.recv().unwrap();
    println!("Getting inclusion proofs...");
    let inclusion_proofs = store_client
        .get_batch_inputs(notes.iter().map(|note| note.id()))
        .await
        .unwrap()
        .note_proofs;

    // Create all accounts and consume txs
    println!("Creating accounts and consuming notes...");
    (0..num_accounts)
        .into_par_iter()
        .map(|index| {
            let init_seed: Vec<_> = index.to_be_bytes().into_iter().chain([0u8; 24]).collect();
            let (new_account, _) = AccountBuilder::new(init_seed.try_into().unwrap())
                .anchor((&genesis_header).try_into().unwrap())
                .account_type(AccountType::RegularAccountImmutableCode)
                .storage_mode(AccountStorageMode::Private)
                .with_component(RpoFalcon512::new(key_pair.public_key()))
                .with_component(BasicWallet)
                .build()
                .unwrap();
            (index, new_account.id())
        })
        .map(|(index, account_id)| {
            let note = notes.get(index).unwrap();
            let inclusion_proof = inclusion_proofs.get(&note.id()).unwrap();
            MockProvenTxBuilder::with_account(account_id, Digest::default(), Digest::default())
                .authenticated_notes(vec![(note.clone(), inclusion_proof.clone())])
                .build()
        })
        .chunks(TRANSACTIONS_PER_BATCH)
        .for_each_with(batch_sender.clone(), |sender, txs| {
            let batch =
                TransactionBatch::new(txs.iter().collect::<Vec<_>>(), Default::default()).unwrap();
            sender.send(batch).unwrap()
        });

    drop(batch_sender);
    db_task.await.unwrap();
    println!("Store loaded in {:?} seconds", load_start.elapsed().as_secs());
}
