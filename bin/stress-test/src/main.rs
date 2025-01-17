use anyhow::Context;
use miden_lib::{
    accounts::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    transaction::TransactionKernel,
};
use miden_node_block_producer::{
    batch_builder::TransactionBatch, block_builder::BlockBuilder, store::StoreClient,
    test_utils::MockProvenTxBuilder,
};
use miden_node_proto::generated::{
    account as proto, requests::SyncStateRequest, store::api_client::ApiClient,
};
use miden_node_store::{config::StoreConfig, server::Store};
use miden_objects::{
    accounts::{AccountBuilder, AccountIdAnchor, AccountStorageMode, AccountType},
    assets::{Asset, FungibleAsset, TokenSymbol},
    crypto::dsa::rpo_falcon512::SecretKey,
    notes::{NoteExecutionMode, NoteTag},
    testing::notes::NoteBuilder,
    transaction::OutputNote,
    Digest, Felt,
};
use miden_processor::crypto::RpoRandomCoin;
use miden_tx::utils::hex_to_bytes;
use rand::Rng;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rayon::prelude::*;
use std::sync::mpsc::channel;
use tokio::task;

#[tokio::main]
async fn main() {
    let store_config = StoreConfig {
        database_filepath: "./loaded-store.sqlite3".into(),
        genesis_filepath: "./genesis.dat".into(),
        blockstore_dir: "./loaded-blocks".into(),
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

    const BATCHES_PER_BLOCK: usize = 16;
    const TRANSACTIONS_PER_BATCH: usize = 16;
    const N_BLOCKS: usize = 7814; // to create 1M acc => 7814 blocks * 16 batches/block * 16 txs/batch * 0.5 acc/tx
    const N_ACCOUNTS: usize = N_BLOCKS * BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH / 2;

    let (batch_sender, batch_receiver) = channel::<TransactionBatch>();

    // Spawn a task for block building
    let db_task = task::spawn(async move {
        let mut current_block: Vec<TransactionBatch> = Vec::with_capacity(BATCHES_PER_BLOCK);
        let mut i = 0;
        while let Ok(batch) = batch_receiver.recv() {
            current_block.push(batch);

            if current_block.len() == BATCHES_PER_BLOCK {
                println!("Building block {}...", i);
                block_builder.build_block(&current_block).await.unwrap();
                current_block.clear();
                i += 1;
            }
        }

        if !current_block.is_empty() {
            block_builder.build_block(&current_block).await.unwrap();
        }
    });

    // Parallel account grinding and batch generation
    (0..N_ACCOUNTS)
        .into_par_iter()
        .map(|i| {
            let (new_account, _) = AccountBuilder::new(init_seed)
                .anchor(AccountIdAnchor::PRE_GENESIS)
                .account_type(AccountType::RegularAccountImmutableCode)
                .storage_mode(AccountStorageMode::Private)
                .with_component(RpoFalcon512::new(key_pair.public_key()))
                .with_component(BasicWallet)
                .build()
                .unwrap();
            if i == 0 {
                println!("Created new account: {}", new_account.id());
            }
            (i, new_account.id())
        })
        .map(|(i, account_id)| {
            let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
            let coin_seed: [u64; 4] = rand::thread_rng().gen();
            let rng: RpoRandomCoin = RpoRandomCoin::new(coin_seed.map(Felt::new));
            let note = NoteBuilder::new(faucet_id, rng)
                .add_assets(vec![asset.clone()])
                .build(&TransactionKernel::assembler())
                .unwrap();
            if i == 0 {
                println!("Created new note: {}", note.id());
            }

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
}

#[tokio::test]
async fn sync_response_time() {
    let store_config = StoreConfig {
        database_filepath: "./loaded-store.sqlite3".into(),
        ..Default::default()
    };

    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });

    // Send sync request and measure performance
    let start = std::time::Instant::now();
    let sync_request = SyncStateRequest {
        block_num: 78,
        note_tags: vec![u32::from(
            NoteTag::from_account_id(
                hex_to_bytes("0xa85900e629b678800000a88b6b7a3f").unwrap().try_into().unwrap(),
                NoteExecutionMode::Local,
            )
            .unwrap(),
        )],
        account_ids: vec![proto::AccountId {
            id: Vec::<u8>::from("0xa85900e629b678800000a88b6b7a3f"),
        }],
        nullifiers: vec![],
    };

    let client = ApiClient::connect(store_config.endpoint.to_string()).await.unwrap();
    client.clone().sync_state(tonic::Request::new(sync_request)).await.unwrap();

    let elapsed = start.elapsed();
    println!("Sync request took: {:?}", elapsed);
}
