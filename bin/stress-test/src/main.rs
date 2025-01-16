use anyhow::Context;
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
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    let store_config = StoreConfig::default();
    let mut join_set = JoinSet::new();

    // Start store
    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    let _ = join_set.spawn(async move { store.serve().await.context("Serving store") }).id();

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

    // The amount of blocks to create and process.
    const BATCHES_PER_BLOCK: usize = 16;
    const TRANSACTIONS_PER_BATCH: usize = 16;
    const N_BLOCKS: usize = 7814; // to create 1M acc => 7814 blocks * 16 batches/block * 16 txs/batch * 0.5 acc/tx

    for block_num in 0..N_BLOCKS {
        let mut batches = Vec::with_capacity(BATCHES_PER_BLOCK);
        for _ in 0..BATCHES_PER_BLOCK {
            let mut batch = Vec::with_capacity(TRANSACTIONS_PER_BATCH);
            for _ in 0..TRANSACTIONS_PER_BATCH / 2 {
                // Create wallet
                let (new_account, _) = AccountBuilder::new(init_seed)
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

                let create_notes_tx = MockProvenTxBuilder::with_account(
                    faucet_id,
                    Digest::default(),
                    Digest::default(),
                )
                .output_notes(vec![OutputNote::Full(note.clone())])
                .build();

                batch.push(create_notes_tx);

                let consume_notes_txs = MockProvenTxBuilder::with_account(
                    account_id,
                    Digest::default(),
                    Digest::default(),
                )
                .unauthenticated_notes(vec![note])
                .build();

                batch.push(consume_notes_txs);
            }
            let batch = TransactionBatch::new(batch.iter().collect::<Vec<_>>(), Default::default())
                .unwrap();

            batches.push(batch);
        }
        println!("Building block {}...", block_num);
        // Inserts the block into the store sending it via StoreClient (RPC)
        block_builder.build_block(&batches).await.unwrap();
    }
}
