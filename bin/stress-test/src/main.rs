use std::{
    path::{Path, PathBuf},
    process::Command as SystemCommand,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use miden_lib::{
    account::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    note::create_p2id_note,
    utils::Serializable,
};
use miden_node_block_producer::{
    batch_builder::TransactionBatch, block_builder::BlockBuilder, store::StoreClient,
    test_utils::MockProvenTxBuilder,
};
use miden_node_proto::{
    domain::note::NoteAuthenticationInfo,
    generated::{
        account as account_proto, requests::SyncStateRequest, store::api_client::ApiClient,
    },
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
    transaction::{OutputNote, ProvenTransaction},
    Digest, Felt,
};
use miden_processor::crypto::{MerklePath, RpoRandomCoin};
use rand::Rng;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task,
};

type AccountNoteTransactions = Vec<(AccountId, Note, ProvenTransaction)>;

const SQLITE_TABLES: [&str; 11] = [
    "account_deltas",
    "block_headers",
    "account_fungible_asset_deltas",
    "notes",
    "account_non_fungible_asset_updates",
    "nullifiers",
    "account_storage_map_updates",
    "settings",
    "account_storage_slot_updates",
    "transactions",
    "accounts",
];

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
        #[arg(short, long, value_name = "NUM_ACCOUNTS", default_value = "10000")]
        num_accounts: usize,

        /// Public accounts percentage
        #[arg(short, long, value_name = "PUBLIC_ACCOUNTS_PERCENTAGE", default_value = "0")]
        public_accounts_percentage: u8,

        /// Path to the genesis file of the store.
        #[arg(short, long, value_name = "GENESIS_FILE", default_value = "./genesis.dat")]
        genesis_file: PathBuf,

        /// Path to the accounts file to dump the created public account ids.
        #[arg(short, long, value_name = "ACCOUNTS_FILE", default_value = "./accounts.txt")]
        accounts_file: PathBuf,
    },
    BenchSyncRequest {
        /// Path to the store database file.
        #[arg(short, long, value_name = "DUMP_FILE", default_value = "./miden-store.sqlite3")]
        dump_file: PathBuf,

        /// Path to the genesis file of the store.
        #[arg(short, long, value_name = "GENESIS_FILE", default_value = "./genesis.dat")]
        genesis_file: PathBuf,

        /// Iterations of the sync request.
        #[arg(short, long, value_name = "ITERATIONS", default_value = "1")]
        iterations: usize,

        /// Path to the accounts file with the dumped public account ids.
        #[arg(short, long, value_name = "ACCOUNTS_FILE", default_value = "./accounts.txt")]
        accounts_file: PathBuf,

        // Block store directory
        #[arg(short, long, value_name = "BLOCKSTORE_DIR", default_value = "./blocks")]
        blockstore_dir: PathBuf,
    },
}

const BATCHES_PER_BLOCK: usize = 16;
const TRANSACTIONS_PER_BATCH: usize = 16;

/// Create and store blocks into the store. Create a given number of accounts, where each account
/// consumes a note created from a faucet. The cli accepts the following parameters:
/// - `dump_file`: Path to the store database file.
/// - `num_accounts`: Number of accounts to create.
/// - `genesis_file`: Path to the genesis file of the store.
#[tokio::main]
async fn main() {
    miden_node_utils::logging::setup_logging().unwrap();

    let cli = Cli::parse();

    match &cli.command {
        Command::SeedStore {
            dump_file,
            num_accounts,
            genesis_file,
            public_accounts_percentage,
            accounts_file,
        } => {
            seed_store(
                dump_file,
                *num_accounts,
                genesis_file,
                *public_accounts_percentage,
                accounts_file,
            )
            .await;
        },
        Command::BenchSyncRequest {
            dump_file,
            genesis_file,
            iterations,
            accounts_file,
            blockstore_dir,
        } => {
            bench_sync_request(dump_file, genesis_file, *iterations, accounts_file, blockstore_dir)
                .await;
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
) {
    let store_config = StoreConfig {
        database_filepath: dump_file.to_path_buf(),
        genesis_filepath: genesis_file.to_path_buf(),
        ..Default::default()
    };

    println!("{:?}", store_config);

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

    // Create first block with the faucet
    let batch = TransactionBatch::new(
        vec![
            &MockProvenTxBuilder::with_account(faucet_id, Digest::default(), Digest::default())
                .output_notes(vec![])
                .build(),
        ],
        NoteAuthenticationInfo::default(),
    )
    .unwrap();

    BlockBuilder::new(store_client.clone()).build_block(&[batch]).await.unwrap();

    // Number of accounts per block
    let num_accounts_per_block = TRANSACTIONS_PER_BATCH * BATCHES_PER_BLOCK;

    // Create sets of accounts and notes
    let accounts_and_notes: Arc<Mutex<AccountNoteTransactions>> =
        Arc::new(Mutex::new(Vec::with_capacity(num_accounts_per_block))); // THIS MIGHT BE REPLACED WITH A STRUCT OR ALIAS

    let start_generating_accounts = create_accounts(
        num_accounts,
        public_accounts_percentage,
        accounts_file,
        genesis_header,
        faucet_id,
        &accounts_and_notes,
    );

    println!(
        "Generated {} accounts in {:.3} seconds",
        accounts_and_notes.lock().unwrap().len(),
        start_generating_accounts.elapsed().as_secs_f64()
    );

    // Each block is composed of [`BATCHES_PER_BLOCK`] batches, and each batch is composed of
    // [`TRANSACTIONS_PER_BATCH`] txs. The first note of the block is always a send assets tx
    // from the faucet to (BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH) - 1 accounts. The rest of
    // the notes are consume note txs from the (BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH) - 1
    // accounts that were minted in the previous block. We should iterate over the total number
    // of blocks needed to create all accounts. For each block, we should create the send assets
    // tx and the consume note txs. And start filling the batches with 16 txs each.
    // We should then build the block using this txs and send it to the store.

    // Spawn the block builder task
    let (batch_sender, batch_receiver) = unbounded_channel::<TransactionBatch>();
    let db_task = task::spawn(build_blocks(batch_receiver, store_client));

    // Create notes
    println!("Creating notes...");
    task::spawn_blocking(move || {
        generate_batches(
            num_accounts,
            faucet_id,
            accounts_and_notes.lock().unwrap().as_slice(),
            &batch_sender,
        );
    })
    .await
    .unwrap();

    let (insertion_time_per_block, total_insertion_time, num_insertions, store_file_size_over_time) =
        db_task.await.unwrap();

    let total_time = start.elapsed().as_secs_f64();

    print_metrics(
        &insertion_time_per_block,
        total_insertion_time,
        num_insertions,
        &store_file_size_over_time,
        total_time,
        dump_file,
    );
}

fn create_accounts(
    num_accounts: usize,
    public_accounts_percentage: u8,
    accounts_file: &Path,
    genesis_header: BlockHeader,
    faucet_id: AccountId,
    accounts_and_notes: &Arc<Mutex<AccountNoteTransactions>>,
) -> Instant {
    // Shared random coin seed and key pair for all accounts
    let coin_seed: [u64; 4] = rand::thread_rng().gen();
    let rng = Arc::new(Mutex::new(RpoRandomCoin::new(coin_seed.map(Felt::new))));
    // Re-using the same key for all accounts to avoid Falcon key generation overhead
    let key_pair = {
        let mut rng = rng.lock().unwrap();
        SecretKey::with_rng(&mut *rng)
    };

    let start_generating_accounts = Instant::now();

    #[allow(clippy::cast_sign_loss, clippy::cast_precision_loss)]
    let public_accounts =
        (num_accounts as f64 * (f64::from(public_accounts_percentage) / 100.0)).round() as usize;
    let private_accounts = num_accounts - public_accounts;

    println!(
        "Creating {private_accounts} private accounts and {public_accounts} public accounts..."
    );

    // Account ID dumper
    let (id_sender, id_receiver) = unbounded_channel::<AccountId>();
    tokio::spawn(dump_account_ids(id_receiver, accounts_file.to_path_buf()));

    // Create the private accounts
    (0..private_accounts).into_par_iter().for_each(|account_index| {
        let account = create_account(
            &genesis_header,
            key_pair.public_key(),
            (account_index) as u64,
            AccountStorageMode::Private,
        );
        let note = {
            let mut rng = rng.lock().unwrap();
            create_note(faucet_id, account.id(), &mut rng)
        };

        let path = MerklePath::new(vec![]);
        let inclusion_proof = NoteInclusionProof::new(0.into(), 0, path).unwrap();

        let consume_tx =
            MockProvenTxBuilder::with_account(account.id(), Digest::default(), Digest::default())
                .authenticated_notes(vec![(note.clone(), inclusion_proof)])
                .account_update_details(AccountUpdateDetails::Private)
                .build();

        accounts_and_notes.lock().unwrap().push((account.id(), note, consume_tx));
    });

    // Create the public accounts
    (0..public_accounts).into_par_iter().for_each(|account_index| {
        let account = create_account(
            &genesis_header,
            key_pair.public_key(),
            (account_index) as u64,
            AccountStorageMode::Public,
        );
        let note = {
            let mut rng = rng.lock().unwrap();
            create_note(faucet_id, account.id(), &mut rng)
        };

        let path = MerklePath::new(vec![]);
        let inclusion_proof = NoteInclusionProof::new(0.into(), 0, path).unwrap();

        let consume_tx =
            MockProvenTxBuilder::with_account(account.id(), account.init_hash(), account.hash())
                .authenticated_notes(vec![(note.clone(), inclusion_proof)])
                .account_update_details(AccountUpdateDetails::New(account.clone()))
                .build();

        accounts_and_notes.lock().unwrap().push((account.id(), note, consume_tx));
        id_sender.send(account.id()).unwrap();
    });
    start_generating_accounts
}

fn print_metrics(
    insertion_time_per_block: &[Duration],
    total_insertion_time: Duration,
    num_insertions: u32,
    store_file_size_over_time: &[u64],
    total_time: f64,
    dump_file: &Path,
) {
    println!(
        "Created notes: inserted {} blocks with avg insertion time {} ms",
        num_insertions,
        (total_insertion_time / num_insertions).as_millis()
    );

    // Print out average insertion time per 1k blocks to track how insertion times increases.
    // Using insertion_time_per_block and taking each 1k blocks to calculate it.
    let mut avg_insertion_time = Duration::default();
    for (i, time) in insertion_time_per_block.iter().enumerate() {
        avg_insertion_time += *time;
        if (i + 1) % 1000 == 0 {
            println!(
                "Inserted from block {} to block {} with avg insertion time {} ms",
                i - 999,
                i,
                (avg_insertion_time / 1000).as_millis()
            );
            avg_insertion_time = Duration::default();
        }
    }

    // Print out the store file size every 50 blocks to track the growth of the file.
    println!("Store file size every 50 blocks:");
    for (i, size) in store_file_size_over_time.iter().enumerate() {
        println!("Block {}: {} bytes", i * 50, size);
    }

    // Print out the average growth rate of the file
    let initial_size = store_file_size_over_time.first().unwrap();
    let final_size = store_file_size_over_time.last().unwrap();

    #[allow(clippy::cast_precision_loss)]
    let growth_rate = (final_size - initial_size) as f64 / f64::from(num_insertions);

    println!("Average growth rate: {growth_rate} bytes per blocks");

    println!("Total time: {total_time:.3} seconds");

    // Apply `VACUUM` to the store to reduce the size of the file by running the command:
    // `sqlite3 miden-store.sqlite3 "VACUUM;"`
    let _ = SystemCommand::new("sqlite3")
        .arg(dump_file)
        .arg("VACUUM;")
        .output()
        .expect("failed to execute process");

    // Then, print out the size of the tables in the store
    for table in &SQLITE_TABLES {
        let db_stats = SystemCommand::new("sqlite3")
            .arg(dump_file)
            .arg(format!(
                "SELECT name, SUM(pgsize) AS size_bytes, (SUM(pgsize) * 1.0) / (SELECT COUNT(*) FROM {table}) AS bytes_per_row FROM dbstat WHERE name = '{table}';"
            ))
            .output()
            .expect("failed to execute process");

        let stdout = String::from_utf8(db_stats.stdout).expect("invalid utf8");
        let stats: Vec<&str> = stdout.trim_end().split('|').collect();
        println!("DB Stats for {}: {} bytes, {} bytes/entry", stats[0], stats[1], stats[2]);
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
fn create_note(faucet_id: AccountId, receipient: AccountId, rng: &mut RpoRandomCoin) -> Note {
    let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
    create_p2id_note(
        faucet_id,
        receipient,
        vec![asset],
        miden_objects::note::NoteType::Public,
        Felt::default(),
        rng,
    )
    .expect("note creation failed")
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
///
/// Returns a tuple containing:
/// - A vector of the time spent on inserting each block.
/// - The total time spent on inserting blocks to the store.
/// - The number of inserted blocks.
/// - A vector containing the store file size every 1k blocks.
async fn build_blocks(
    mut batch_receiver: UnboundedReceiver<TransactionBatch>,
    store_client: StoreClient,
) -> (Vec<Duration>, Duration, u32, Vec<u64>) {
    let block_builder = BlockBuilder::new(store_client);

    let mut current_block: Vec<TransactionBatch> = Vec::with_capacity(BATCHES_PER_BLOCK);
    let mut insertion_time_per_block = Vec::new();
    // Keep track of the store file size every 1k blocks in a vector to track the growth of the
    // file.
    let mut store_file_sizes = Vec::new();
    // Store the file size of the store before starting the insertion.
    let store_file_size = std::fs::metadata("./miden-store.sqlite3").unwrap().len();
    store_file_sizes.push(store_file_size);

    let mut counter = 0;
    while let Some(batch) = batch_receiver.recv().await {
        current_block.push(batch);

        if current_block.len() == BATCHES_PER_BLOCK {
            let start = Instant::now();
            block_builder.build_block(&current_block).await.unwrap();
            insertion_time_per_block.push(start.elapsed());
            current_block.clear();

            // We track the size of the DB every 50 blocks.
            if counter % 50 == 0 {
                let store_file_size = std::fs::metadata("./miden-store.sqlite3").unwrap().len();
                let wal_file_size = std::fs::metadata("./miden-store.sqlite3-wal").unwrap().len();
                store_file_sizes.push(store_file_size + wal_file_size);
            }

            counter += 1;
        }
    }

    if !current_block.is_empty() {
        let start = Instant::now();
        block_builder.build_block(&current_block).await.unwrap();
        insertion_time_per_block.push(start.elapsed());
    }

    let num_insertions = insertion_time_per_block.len() as u32;
    let total_insertion_time: Duration = insertion_time_per_block.iter().sum();
    (insertion_time_per_block, total_insertion_time, num_insertions, store_file_sizes)
}

/// Generate batches of transactions to be inserted into the store.
/// The first transaction in each batch sends assets from the faucet to 255 accounts.
/// The rest of the transactions consume the notes created by the faucet in the previous block.
fn generate_batches(
    num_accounts: usize,
    faucet_id: AccountId,
    accounts_and_notes: &[(AccountId, Note, ProvenTransaction)],
    batch_sender: &UnboundedSender<TransactionBatch>,
) {
    let mut accounts_notes_txs_1 = vec![];

    let consumes_per_block = (BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH) - 1;
    let total_blocks = (num_accounts / consumes_per_block) + 1; // +1 to account for the first block with the send assets tx only

    for i in 0..total_blocks {
        let start = i * consumes_per_block;
        let end = ((i * consumes_per_block) + consumes_per_block).min(num_accounts);
        let accounts_notes_txs_0 = accounts_and_notes[start..end].to_vec();
        let mut txs = Vec::with_capacity(BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH);

        // Create the send assets tx
        let mint_assets =
            MockProvenTxBuilder::with_account(faucet_id, Digest::default(), Digest::default())
                .output_notes(
                    accounts_notes_txs_0
                        .iter()
                        .map(|(_, note, _)| OutputNote::Full(note.clone()))
                        .collect(),
                )
                .build();

        txs.push(mint_assets);

        // Create the consume note txs
        accounts_notes_txs_1.iter().take(consumes_per_block).for_each(
            |(_, _, tx): &(AccountId, Note, ProvenTransaction)| {
                txs.push(tx.clone());
            },
        );

        // Fill the batches with [TRANSACTIONS_PER_BATCH] txs each
        txs.chunks(TRANSACTIONS_PER_BATCH).for_each(|txs| {
            let batch = TransactionBatch::new(
                txs.iter().collect::<Vec<_>>(),
                NoteAuthenticationInfo::default(),
            )
            .unwrap();
            batch_sender.send(batch).unwrap();
        });

        accounts_notes_txs_1 = accounts_notes_txs_0;
    }
}

/// Sends a sync request to the store and measures the performance.
async fn bench_sync_request(
    dump_file: &Path,
    genesis_file: &Path,
    iterations: usize,
    accounts_file: &Path,
    blockstore_dir: &Path,
) {
    let store_config = StoreConfig {
        database_filepath: dump_file.to_path_buf(),
        genesis_filepath: genesis_file.to_path_buf(),
        blockstore_dir: blockstore_dir.to_path_buf(),
        ..Default::default()
    };

    println!("{:?}", store_config);

    // Start store
    let store = Store::with_existing_db(store_config.clone())
        .await
        .context("Loading store")
        .unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });
    let start = Instant::now();

    let mut api_client = ApiClient::connect(store_config.endpoint.to_string()).await.unwrap();

    // Send sync request and measure performance
    // Read the account ids from the file
    // If iterations > accounts_file, repeat the account ids
    let accounts = tokio::fs::read_to_string(accounts_file).await.unwrap();
    let accounts: Vec<&str> = accounts.lines().collect();

    let mut account_ids = accounts.iter().cycle();

    for _ in 0..iterations {
        let account_id = account_ids.next().unwrap();
        send_sync_request(&mut api_client, account_id).await;
    }

    let elapsed = start.elapsed();
    println!("Sync request took: {elapsed:?}");
}

async fn send_sync_request(
    api_client: &mut ApiClient<tonic::transport::Channel>,
    account_id: &str,
) {
    let account_id = AccountId::from_hex(account_id).unwrap();
    let sync_request = SyncStateRequest {
        block_num: 0,
        note_tags: vec![u32::from(
            NoteTag::from_account_id(account_id, NoteExecutionMode::Local).unwrap(),
        )],
        account_ids: vec![account_proto::AccountId { id: account_id.to_bytes() }],
        nullifiers: vec![],
    };

    assert!(api_client.sync_state(sync_request).await.is_ok());
}

async fn dump_account_ids(mut receiver: UnboundedReceiver<AccountId>, file: PathBuf) {
    let mut file = tokio::fs::File::create(file).await.unwrap();
    while let Some(account_id) = receiver.recv().await {
        file.write_all(format!("{account_id}\n").as_bytes()).await.unwrap();
    }
}
