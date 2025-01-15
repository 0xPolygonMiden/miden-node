use anyhow::Context;
use miden_lib::{
    accounts::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    transaction::TransactionKernel,
};
use miden_node_block_producer::{
    batch_builder::TransactionBatch, config::BlockProducerConfig, server::BlockProducer,
    test_utils::MockProvenTxBuilder,
};
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
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    let block_producer_config = BlockProducerConfig::default();
    let store_config = StoreConfig::default();
    let mut join_set = JoinSet::new();

    // Start store
    let store = Store::init(store_config).await.context("Loading store").unwrap();
    let _ = join_set.spawn(async move { store.serve().await.context("Serving store") }).id();

    // Start block-producer
    // TODO: is the full the BlockProducer needed? we should instantiate only a BlockBuilder
    let block_producer = BlockProducer::init(block_producer_config)
        .await
        .context("Loading block-producer")
        .unwrap();

    println!("Creating new faucet account...");
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    let key_pair = SecretKey::with_rng(&mut rng);
    let init_seed = [0_u8; 32];
    let (new_faucet, _seed) = AccountBuilder::new()
        .init_seed(init_seed)
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

    // The amount of blocks to create and process.
    const BATCHES_PER_BLOCK: usize = 4;
    const N_BLOCKS: usize = 250_000;
    // 250_000 blocks * 4 batches/block * 1 accounts/batch = 1_000_000 accounts
    // Each batch contains 2 txs: one to create a note and another to consume it.

    for block_num in 0..N_BLOCKS {
        let mut batches = Vec::with_capacity(BATCHES_PER_BLOCK);
        for _ in 0..BATCHES_PER_BLOCK {
            // Create wallet
            let (new_account, _) = AccountBuilder::new()
                .init_seed(init_seed)
                .anchor(AccountIdAnchor::PRE_GENESIS)
                .account_type(AccountType::RegularAccountImmutableCode)
                .storage_mode(AccountStorageMode::Private)
                .with_component(RpoFalcon512::new(key_pair.public_key()))
                .with_component(BasicWallet)
                .build()
                .unwrap();
            let account_id = new_account.id();

            // Create note
            let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
            let coin_seed: [u64; 4] = rand::thread_rng().gen();
            let rng: RpoRandomCoin = RpoRandomCoin::new(coin_seed.map(Felt::new));
            let note = NoteBuilder::new(faucet_id, rng)
                .add_assets(vec![asset.clone()])
                .build(&TransactionKernel::assembler())
                .unwrap();

            let batch = {
                // First tx: create the note
                let create_notes_tx = MockProvenTxBuilder::with_account(
                    faucet_id,
                    Digest::default(),
                    Digest::default(),
                )
                .output_notes(vec![OutputNote::Full(note.clone())])
                .build();

                // Second tx: consume the note
                let consume_notes_txs = MockProvenTxBuilder::with_account(
                    account_id,
                    Digest::default(),
                    Digest::default(),
                )
                .unauthenticated_notes(vec![note])
                .build();

                TransactionBatch::new([&create_notes_tx, &consume_notes_txs], Default::default())
                    .unwrap()
            };
            batches.push(batch);
        }
        println!("Building block {}...", block_num);
        // Inserts the block into the store sending it via StoreClient (RPC)
        block_producer.block_builder.build_block(&batches).await.unwrap();
    }
}
