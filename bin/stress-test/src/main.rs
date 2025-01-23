use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use miden_lib::{
    account::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    transaction::TransactionKernel,
    utils::Serializable,
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
    account::{
        delta::AccountUpdateDetails, Account, AccountBuilder, AccountId, AccountStorageMode,
        AccountType,
    },
    asset::{Asset, FungibleAsset, TokenSymbol},
    block::BlockHeader,
    crypto::dsa::rpo_falcon512::{PublicKey, SecretKey},
    note::{Note, NoteExecutionMode, NoteInclusionProof, NoteTag},
    testing::note::NoteBuilder,
    transaction::OutputNote,
    Digest, Felt, MAX_OUTPUT_NOTES_PER_BATCH,
};
use miden_processor::crypto::{MerklePath, RpoRandomCoin};
use rand::Rng;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task,
};

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create and store blocks into the store. Create a given number of accounts, where each
    /// account consumes a note created from a faucet.
    SeedStore {
        /// Path to the store database file.
        #[arg(short, long, value_name = "DUMP_FILE", default_value = "./miden-store.sqlite3")]
        dump_file: PathBuf,

        /// Number of accounts to create.
        #[arg(short, long, value_name = "NUM_ACCOUNTS")]
        num_accounts: usize,

        /// Percentage of public accounts to total accounts to create.
        #[arg(short, long, value_name = "PUBLIC_ACCOUNTS_PERCENTAGE", default_value = "0")]
        public_accounts_percentage: u8,

        /// Path to the genesis file of the store.
        #[arg(short, long, value_name = "GENESIS_FILE")]
        genesis_file: PathBuf,

        /// Path to the accounts file to dump the created public account ids.
        #[arg(short, long, value_name = "ACCOUNTS_FILE", default_value = "accounts.csv")]
        accounts_file: PathBuf,

        /// Path to the insertion time file to dump the insertion times.
        #[arg(
            short,
            long,
            value_name = "INSERTION_TIME_FILE",
            default_value = "insertion_times.csv"
        )]
        insertion_time_file: PathBuf,
    },

    BenchSyncRequest {
        /// Path to the store database file.
        #[arg(short, long, value_name = "DUMP_FILE", default_value = "./miden-store.sqlite3")]
        dump_file: PathBuf,

        /// Path to the genesis file of the store.
        #[arg(short, long, value_name = "GENESIS_FILE")]
        genesis_file: PathBuf,
    },
}

const BATCHES_PER_BLOCK: usize = 16;
const TRANSACTIONS_PER_BATCH: usize = 16;
const NOTES_PER_TRANSACTION: usize = MAX_OUTPUT_NOTES_PER_BATCH / TRANSACTIONS_PER_BATCH;

/// Create and store blocks into the store. Create a given number of accounts, where each account
/// consumes a note created from a faucet. The cli accepts the following parameters:
/// - `dump_file`: Path to the store database file.
/// - `num_accounts`: Number of accounts to create.
/// - `genesis_file`: Path to the genesis file of the store.
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Command::SeedStore {
            dump_file,
            num_accounts,
            genesis_file,
            public_accounts_percentage,
            accounts_file,
            insertion_time_file,
        } => {
            seed_store(
                dump_file,
                *num_accounts,
                genesis_file,
                *public_accounts_percentage,
                accounts_file,
                insertion_time_file,
            )
            .await;
        },
        Command::BenchSyncRequest { dump_file, genesis_file } => {
            bench_sync_request(dump_file, genesis_file).await;
        },
    }
}

/// Seed the store with a given number of accounts.
async fn seed_store(
    dump_file: &Path,
    num_accounts: usize,
    genesis_file: &Path,
    public_accounts_percentage: u8,
    accounts_file: &Path,
    insertion_time_file: &Path,
) {
    let store_config = StoreConfig {
        database_filepath: dump_file.to_path_buf(),
        genesis_filepath: genesis_file.to_path_buf(),
        ..Default::default()
    };

    // Start store
    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });
    let start = Instant::now();

    // Create faucet
    println!("Creating new faucet account...");
    let store_client =
        StoreClient::new(ApiClient::connect(store_config.endpoint.to_string()).await.unwrap());
    let genesis_header = store_client.latest_header().await.unwrap();
    let faucet_id = create_faucet(&genesis_header);

    // Spawn first block builder task
    let (batch_sender, batch_receiver) = unbounded_channel::<TransactionBatch>();
    let db_task = task::spawn(build_blocks(batch_receiver, store_client));

    // Create notes
    println!("Creating notes...");
    let notes = task::spawn_blocking(move || {
        generate_note_batches(num_accounts, faucet_id, batch_sender.clone())
    })
    .await
    .unwrap();
    let insertion_times = db_task.await.unwrap();
    let num_insertions = insertion_times.len() as u32;
    let insertion_time: Duration = insertion_times.iter().sum();
    println!(
        "Created notes: inserted {} blocks with avg insertion time {:?}",
        num_insertions,
        insertion_time / num_insertions
    );

    // Spawn second block builder task
    let store_client =
        StoreClient::new(ApiClient::connect(store_config.endpoint.to_string()).await.unwrap());
    let (batch_sender, batch_receiver) = unbounded_channel::<TransactionBatch>();
    let db_task = task::spawn(build_blocks(batch_receiver, store_client));

    // Create accounts to consume the notes
    println!("Creating accounts and consuming notes...");
    let account_file = accounts_file.to_path_buf();
    task::spawn_blocking(move || {
        generate_account_batches(
            num_accounts,
            &notes,
            batch_sender,
            &genesis_header,
            public_accounts_percentage,
            account_file,
        );
    })
    .await
    .unwrap();
    let insertion_times = db_task.await.unwrap();
    let num_insertions = insertion_times.len() as u32;
    let insertion_time: Duration = insertion_times.iter().sum();
    println!(
        "Consumed notes: inserted {} blocks with avg insertion time {:?}",
        num_insertions,
        insertion_time / num_insertions
    );

    dump_insertion_times(insertion_time_file.to_path_buf(), insertion_times);
    println!("Store loaded in {:?}", start.elapsed());
}

/// Dump the insertion times to a file.
fn dump_insertion_times(file_path: PathBuf, insertion_times: Vec<Duration>) {
    let mut file = File::create(file_path).unwrap();
    writeln!(file, "insertion_time_ms").unwrap();
    for time in insertion_times {
        writeln!(file, "{}", time.as_millis()).unwrap();
    }
}

/// Dump the account ids to a file.
async fn dump_account_ids(mut receiver: UnboundedReceiver<AccountId>, file: PathBuf) {
    let mut file = tokio::fs::File::create(file).await.unwrap();
    file.write_all("account_id\n".as_bytes()).await.unwrap();
    while let Some(account_id) = receiver.recv().await {
        file.write_all(format!("{}\n", account_id).as_bytes()).await.unwrap();
    }
}

/// Create a new faucet account with a given anchor block.
fn create_faucet(anchor_block: &BlockHeader) -> AccountId {
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

/// Create a new note containing 10 tokens of the fungible asset associated with the specified
/// `faucet_id`.
fn create_note(faucet_id: AccountId) -> Note {
    let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    NoteBuilder::new(faucet_id, rng)
        .add_assets(vec![asset])
        .build(&TransactionKernel::assembler())
        .unwrap()
}

/// Create a new account with a given public key and anchor block. Generates the seed from the given
/// index.
fn create_account(
    anchor_block: &BlockHeader,
    public_key: PublicKey,
    index: u64,
    storage_mode: AccountStorageMode,
) -> Account {
    let init_seed: Vec<_> = index.to_be_bytes().into_iter().chain([0u8; 24]).collect();

    let (new_account, _) = AccountBuilder::new(init_seed.try_into().unwrap())
        .anchor(anchor_block.try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(storage_mode)
        .with_component(RpoFalcon512::new(public_key))
        .with_component(BasicWallet)
        .build()
        .unwrap();
    new_account
}

/// Build blocks from transaction batches. Each new block contains [`BATCHES_PER_BLOCK`] batches.
/// Returns the total time spent on inserting blocks to the store and the number of inserted blocks.
async fn build_blocks(
    mut batch_receiver: UnboundedReceiver<TransactionBatch>,
    store_client: StoreClient,
) -> Vec<Duration> {
    let block_builder = BlockBuilder::new(store_client);

    let mut current_block: Vec<TransactionBatch> = Vec::with_capacity(BATCHES_PER_BLOCK);
    let mut insertion_times = Vec::new();
    while let Some(batch) = batch_receiver.recv().await {
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
    insertion_times
}

/// Create a given number of notes and group them into transactions and batches.
/// The batches are sent to the block builder.
fn generate_note_batches(
    num_notes: usize,
    faucet_id: AccountId,
    batch_sender: UnboundedSender<TransactionBatch>,
) -> Vec<Note> {
    let notes: Vec<Note> = (0..num_notes).into_par_iter().map(|_| create_note(faucet_id)).collect();
    // TODO: dump the notes into a file instead of keeping them in memory?
    notes
        .clone()
        .into_par_iter()
        .chunks(NOTES_PER_TRANSACTION)
        .map(|note_chunk| {
            MockProvenTxBuilder::with_account(faucet_id, Digest::default(), Digest::default())
                .output_notes(
                    note_chunk.iter().map(|note| OutputNote::Full(note.clone())).collect(),
                )
                .build()
        })
        .chunks(TRANSACTIONS_PER_BATCH)
        .for_each_with(batch_sender, |sender, txs| {
            let batch =
                TransactionBatch::new(txs.iter().collect::<Vec<_>>(), Default::default()).unwrap();
            sender.send(batch).unwrap()
        });

    notes
}

/// Grinds accounts, and for each one create a transaction that consumes a note.
/// Groups the created transactions into batches and sends them to the block builder.
fn generate_account_batches(
    num_accounts: usize,
    notes: &[Note],
    batch_sender: UnboundedSender<TransactionBatch>,
    genesis_header: &BlockHeader,
    public_accounts_percentage: u8,
    accounts_file: PathBuf,
) {
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    // Re-using the same key for all accounts to avoid Falcon key generation overhead
    let key_pair = SecretKey::with_rng(&mut rng);

    let (id_sender, id_receiver) = unbounded_channel::<AccountId>();
    tokio::spawn(dump_account_ids(id_receiver, accounts_file));

    (0..num_accounts)
        .into_par_iter()
        .map_with(id_sender, |sender, index| {
            let storage_mode = if index >= num_accounts * public_accounts_percentage as usize / 100
            {
                AccountStorageMode::Private
            } else {
                AccountStorageMode::Public
            };
            let account =
                create_account(genesis_header, key_pair.public_key(), index as u64, storage_mode);
            sender.send(account.id()).unwrap();
            account
        })
        .enumerate()
        .map(|(index, account)| {
            let account_id = account.id();
            let note = notes.get(index).unwrap().clone();

            let path = MerklePath::new(vec![]);
            let inclusion_proof = NoteInclusionProof::new(0.into(), 0, path).unwrap();

            let account_update_details =
                if index >= num_accounts * public_accounts_percentage as usize / 100 {
                    AccountUpdateDetails::Private
                } else {
                    AccountUpdateDetails::New(account.clone())
                };

            MockProvenTxBuilder::with_account(account_id, Digest::default(), account.hash())
                .authenticated_notes(vec![(note, inclusion_proof)])
                .account_update_details(account_update_details)
                .build()
        })
        .chunks(TRANSACTIONS_PER_BATCH)
        .for_each_with(batch_sender, |sender, txs| {
            let batch =
                TransactionBatch::new(txs.iter().collect::<Vec<_>>(), Default::default()).unwrap();
            sender.send(batch).unwrap();
        });
}

/// Sends a sync request to the store and measures the performance.
async fn bench_sync_request(database_file: &Path, genesis_file: &Path) {
    let store_config = StoreConfig {
        database_filepath: database_file.to_path_buf(),
        genesis_filepath: genesis_file.to_path_buf(),
        ..Default::default()
    };

    // Start store
    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });
    let start = Instant::now();

    // Send sync request and measure performance
    // TODO: read account id from the accounts file
    let account_id = AccountId::from_hex("0x9eb0c314a717bd000000d30140dcc0").unwrap();
    let sync_request = SyncStateRequest {
        block_num: 0,
        note_tags: vec![u32::from(
            NoteTag::from_account_id(account_id, NoteExecutionMode::Local).unwrap(),
        )],
        account_ids: vec![proto::AccountId { id: account_id.to_bytes() }],
        nullifiers: vec![],
    };

    let api_client = ApiClient::connect(store_config.endpoint.to_string()).await.unwrap();
    api_client.clone().sync_state(sync_request).await.unwrap();

    let elapsed = start.elapsed();
    println!("Sync request took: {:?}", elapsed);
}
