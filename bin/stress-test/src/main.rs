use std::{
    env::temp_dir,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod metrics;
use anyhow::Context;
use clap::{Parser, Subcommand};
use metrics::Metrics;
use miden_air::HashFunction;
use miden_block_prover::LocalBlockProver;
use miden_lib::{
    account::{auth::RpoFalcon512, faucets::BasicFungibleFaucet, wallets::BasicWallet},
    note::create_p2id_note,
    utils::Serializable,
};
use miden_node_block_producer::store::StoreClient;
use miden_node_proto::generated::store::api_client::ApiClient;
use miden_node_store::{config::StoreConfig, genesis::GenesisState, server::Store};
use miden_node_utils::tracing::grpc::OtelInterceptor;
use miden_objects::{
    Digest, Felt,
    account::{
        Account, AccountBuilder, AccountId, AccountIdAnchor, AccountStorageMode, AccountType,
    },
    asset::{Asset, FungibleAsset, TokenSymbol},
    batch::{BatchAccountUpdate, BatchId, ProvenBatch},
    block::{BlockHeader, BlockNumber, ProposedBlock},
    crypto::dsa::rpo_falcon512::{PublicKey, SecretKey},
    note::{Note, NoteHeader, NoteInclusionProof},
    transaction::{InputNote, InputNotes, OutputNote, ProvenTransaction, ProvenTransactionBuilder},
    vm::ExecutionProof,
};
use miden_processor::crypto::{MerklePath, RpoRandomCoin};
use rand::Rng;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tokio::{fs, task};
use winterfell::Proof;

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
    let cli = Cli::parse();

    match &cli.command {
        Command::SeedStore { dump_file, num_accounts } => {
            seed_store(dump_file, *num_accounts).await;
        },
    }
}

/// Seed the store with a given number of accounts.
async fn seed_store(dump_file: &Path, num_accounts: usize) {
    let start = Instant::now();

    // Generate the faucet account and the genesis state
    let faucet = create_faucet();
    let genesis_state = GenesisState::new(vec![faucet.clone()], 1, 1_672_531_200);
    let genesis_filepath = temp_dir().join("genesis.dat");
    fs::write(genesis_filepath.clone(), genesis_state.to_bytes()).await.unwrap();

    // Start store
    let store_config = StoreConfig {
        database_filepath: dump_file.to_path_buf(),
        genesis_filepath,
        ..Default::default()
    };
    let store = Store::init(store_config.clone()).await.context("Loading store").unwrap();
    task::spawn(async move { store.serve().await.context("Serving store") });
    let channel = tonic::transport::Endpoint::try_from(store_config.endpoint.to_string())
        .unwrap()
        .connect()
        .await
        .unwrap();
    let store_api_client = ApiClient::with_interceptor(channel, OtelInterceptor);
    let store_client = StoreClient::new(store_api_client);

    // Start generating blocks
    let genesis_header = genesis_state.into_block().unwrap().header();
    let metrics =
        generate_blocks(num_accounts, faucet, &genesis_header, &store_client, dump_file).await;

    println!("Total time: {:.3} seconds", start.elapsed().as_secs_f64());
    println!("{metrics}");
}

/// Generate batches of transactions to be inserted into the store.
/// The first transaction in each batch sends assets from the faucet to 255 accounts.
/// The rest of the transactions consume the notes created by the faucet in the previous block.
async fn generate_blocks(
    num_accounts: usize,
    faucet: Account,
    genesis_header: &BlockHeader,
    store_client: &StoreClient,
    dump_file: &Path,
) -> Metrics {
    // Each block is composed of [`BATCHES_PER_BLOCK`] batches, and each batch is composed of
    // [`TRANSACTIONS_PER_BATCH`] txs. The first note of the block is always a send assets tx
    // from the faucet to (BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH) - 1 accounts. The rest of
    // the notes are consume note txs from the (BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH) - 1
    // accounts that were minted in the previous block. We should iterate over the total number
    // of blocks needed to create all accounts. For each block, we should create the send assets
    // tx and the consume note txs. And start filling the batches with 16 txs each.
    // We should then build the block using this txs and send it to the store.
    let mut metrics = Metrics::new(dump_file.to_path_buf(), get_store_size(dump_file));

    let mut consume_notes_txs = vec![];

    let consumes_per_block = TRANSACTIONS_PER_BATCH * BATCHES_PER_BLOCK - 1;
    let total_blocks = (num_accounts / consumes_per_block) + 1; // +1 to account for the first block with the send assets tx only

    // Shared random coin seed and key pair for all accounts to avoid key generation overhead
    let coin_seed: [u64; 4] = rand::thread_rng().r#gen();
    let rng = Arc::new(Mutex::new(RpoRandomCoin::new(coin_seed.map(Felt::new))));
    let key_pair = {
        let mut rng = rng.lock().unwrap();
        SecretKey::with_rng(&mut *rng)
    };

    for i in 0..total_blocks {
        let mut block_txs = Vec::with_capacity(BATCHES_PER_BLOCK * TRANSACTIONS_PER_BATCH);

        // Create accounts and notes that mint assets
        let accounts_and_notes = create_accounts_and_notes(
            genesis_header,
            &key_pair,
            &rng,
            faucet.id(),
            consumes_per_block,
        );

        // Create the tx that creates the notes
        let initial_account_hash = if i == 0 {
            faucet.hash()
        } else {
            [i as u64; 4].try_into().unwrap()
        };
        let mint_assets_tx = create_tx(
            faucet.id(),
            initial_account_hash,
            [(i + 1) as u64; 4].try_into().unwrap(),
            accounts_and_notes.iter().map(|(_, note, _)| note.clone()).collect(),
            vec![],
        );

        // Collect all the txs
        block_txs.push(mint_assets_tx);
        block_txs.extend(
            consume_notes_txs
                .iter()
                .map(|(_, _, tx): &(AccountId, Note, ProvenTransaction)| tx.clone()),
        );

        // Create the batches with [TRANSACTIONS_PER_BATCH] txs each
        let batches: Vec<ProvenBatch> =
            block_txs.chunks(TRANSACTIONS_PER_BATCH).map(create_batch).collect();

        consume_notes_txs = accounts_and_notes;

        // Create the block and send it to the store
        let (elapsed, block_size) = apply_block(batches.clone(), store_client).await;

        // Track metrics
        metrics.add_insertion(elapsed, block_size);
        if i % 50 == 0 {
            metrics.add_store_size(get_store_size(dump_file));
        }
    }
    metrics
}

/// Given a list of batches, create a `ProvenBlock` and send it to the store.
/// Returns a tuple with:
/// - the time spent on executing `StoreClient::apply_block`
/// - the size of the block in bytes
async fn apply_block(batches: Vec<ProvenBatch>, store_client: &StoreClient) -> (Duration, usize) {
    let inputs = store_client
        .get_block_inputs(
            batches.iter().flat_map(ProvenBatch::updated_accounts),
            batches.iter().flat_map(ProvenBatch::created_nullifiers),
            batches.iter().flat_map(|batch| {
                batch
                    .input_notes()
                    .iter()
                    .cloned()
                    .filter_map(|note| note.header().map(NoteHeader::id))
            }),
            batches.iter().map(ProvenBatch::reference_block_num),
        )
        .await
        .unwrap();
    let proposed_block = ProposedBlock::new(inputs, batches).unwrap();
    let proven_block = LocalBlockProver::new(0)
        .prove_without_batch_verification(proposed_block)
        .unwrap();

    let start = Instant::now();
    let block_size: usize = proven_block.to_bytes().len();
    store_client.apply_block(&proven_block).await.unwrap();
    (start.elapsed(), block_size)
}

// HELPERS
// -------------------------------------------------------------------------------------------------

/// Create accounts and notes that mint assets for a given number of accounts.
///
/// Returns a vector of tuples, where each tuple contains:
/// - the new account id
/// - the new note that sends assets to the account
/// - the proven transaction that consumes the note
fn create_accounts_and_notes(
    genesis_header: &BlockHeader,
    key_pair: &SecretKey,
    rng: &Arc<Mutex<RpoRandomCoin>>,
    faucet_id: AccountId,
    num_accounts: usize,
) -> Vec<(AccountId, Note, ProvenTransaction)> {
    (0..num_accounts)
        .into_par_iter()
        .map(|account_index| {
            let account =
                create_account(genesis_header, key_pair.public_key(), account_index as u64);
            let note = {
                let mut rng = rng.lock().unwrap();
                create_note(faucet_id, account, &mut rng)
            };

            let path = MerklePath::new(vec![]);
            let inclusion_proof = NoteInclusionProof::new(0.into(), 0, path).unwrap();

            let consume_tx = create_tx(
                account,
                Digest::default(),
                Digest::default(),
                vec![],
                vec![(note.clone(), inclusion_proof)],
            );
            (account, note, consume_tx)
        })
        .collect()
}

/// Create a new note containing 10 tokens of the fungible asset associated with the specified
/// `faucet_id`.
fn create_note(faucet_id: AccountId, recipient: AccountId, rng: &mut RpoRandomCoin) -> Note {
    let asset = Asset::Fungible(FungibleAsset::new(faucet_id, 10).unwrap());
    create_p2id_note(
        faucet_id,
        recipient,
        vec![asset],
        miden_objects::note::NoteType::Public,
        Felt::default(),
        rng,
    )
    .expect("note creation failed")
}

/// Create a new account with a given public key and anchor block. Generates the seed from the given
/// index.
fn create_account(anchor_block: &BlockHeader, public_key: PublicKey, index: u64) -> AccountId {
    let init_seed: Vec<_> = index.to_be_bytes().into_iter().chain([0u8; 24]).collect();
    let (new_account, _) = AccountBuilder::new(init_seed.try_into().unwrap())
        .anchor(anchor_block.try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_component(RpoFalcon512::new(public_key))
        .with_component(BasicWallet)
        .build()
        .unwrap();
    new_account.id()
}

/// Get the size of the store file and its WAL file.
fn get_store_size(dump_file: &Path) -> u64 {
    let store_file_size = std::fs::metadata(dump_file).unwrap().len();
    let wal_file_size =
        std::fs::metadata(format!("{}-wal", dump_file.to_str().unwrap())).unwrap().len();
    store_file_size + wal_file_size
}

/// Create a new faucet account.
fn create_faucet() -> Account {
    let coin_seed: [u64; 4] = rand::thread_rng().r#gen();
    let mut rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    let key_pair = SecretKey::with_rng(&mut rng);
    let init_seed = [0_u8; 32];

    let (new_faucet, _seed) = AccountBuilder::new(init_seed)
        .anchor(AccountIdAnchor::PRE_GENESIS)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Private)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(
            BasicFungibleFaucet::new(TokenSymbol::new("TEST").unwrap(), 2, Felt::new(u64::MAX))
                .unwrap(),
        )
        .build()
        .unwrap();
    new_faucet
}

fn create_batch(txs: &[ProvenTransaction]) -> ProvenBatch {
    let account_updates = txs
        .iter()
        .map(|tx| (tx.account_id(), BatchAccountUpdate::from_transaction(tx)))
        .collect();
    let output_notes = txs.iter().flat_map(|tx| tx.output_notes().iter().cloned()).collect();
    let input_notes = txs.iter().flat_map(|tx| tx.input_notes().iter().cloned()).collect();
    ProvenBatch::new_unchecked(
        BatchId::from_transactions(txs.iter()),
        Digest::default(),
        BlockNumber::GENESIS,
        account_updates,
        InputNotes::new_unchecked(input_notes),
        output_notes,
        BlockNumber::from(u32::MAX),
    )
}

fn create_tx(
    account_id: AccountId,
    initial_account_hash: Digest,
    final_account_hash: Digest,
    output_notes: Vec<Note>,
    input_notes: Vec<(Note, NoteInclusionProof)>,
) -> ProvenTransaction {
    ProvenTransactionBuilder::new(
        account_id,
        initial_account_hash,
        final_account_hash,
        BlockNumber::from(0),
        Digest::default(),
        u32::MAX.into(),
        ExecutionProof::new(Proof::new_dummy(), HashFunction::Blake3_192),
    )
    .add_input_notes(
        input_notes
            .into_iter()
            .map(|(note, proof)| InputNote::authenticated(note, proof))
            .collect::<Vec<InputNote>>(),
    )
    .add_output_notes(output_notes.into_iter().map(OutputNote::Full).collect::<Vec<OutputNote>>())
    .build()
    .unwrap()
}
