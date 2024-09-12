use std::{cell::RefCell, rc::Rc, time::Duration};

use miden_lib::{
    accounts::faucets::create_basic_fungible_faucet, notes::create_p2id_note,
    transaction::TransactionKernel, AuthScheme,
};
use miden_node_proto::generated::{
    requests::{GetBlockHeaderByNumberRequest, SubmitProvenTransactionRequest},
    rpc::api_client::ApiClient,
};
use miden_objects::{
    accounts::{Account, AccountDelta, AccountId, AccountStorageMode, AuthSecretKey},
    assets::{FungibleAsset, TokenSymbol},
    crypto::{
        dsa::rpo_falcon512::SecretKey,
        merkle::{MmrPeaks, PartialMmr},
        rand::RpoRandomCoin,
    },
    notes::{Note, NoteId, NoteType},
    transaction::{ChainMmr, ExecutedTransaction, InputNotes, TransactionArgs, TransactionScript},
    vm::AdviceMap,
    BlockHeader, Felt, Word,
};
use miden_tx::{
    auth::BasicAuthenticator, utils::Serializable, DataStore, DataStoreError, ProvingOptions,
    TransactionExecutor, TransactionInputs, TransactionProver,
};
use rand::{rngs::StdRng, thread_rng, Rng};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use tonic::transport::Channel;

use crate::{config::FaucetConfig, errors::FaucetError};

pub const DISTRIBUTE_FUNGIBLE_ASSET_SCRIPT: &str =
    include_str!("transaction_scripts/distribute_fungible_asset.masm");

// FAUCET CLIENT
// ================================================================================================

/// Basic client that handles execution, proving and submitting of mint transactions
/// for the faucet.
pub struct FaucetClient {
    rpc_api: ApiClient<Channel>,
    executor: TransactionExecutor<FaucetDataStore, BasicAuthenticator<StdRng>>,
    data_store: FaucetDataStore,
    id: AccountId,
    rng: RpoRandomCoin,
}

unsafe impl Send for FaucetClient {}
unsafe impl Sync for FaucetClient {}

impl FaucetClient {
    pub async fn new(config: FaucetConfig) -> Result<Self, FaucetError> {
        let (rpc_api, root_block_header, root_chain_mmr) =
            initialize_faucet_client(config.clone()).await?;

        let (faucet_account, account_seed, secret) = build_account(config.clone())?;
        let faucet_account = Rc::new(RefCell::new(faucet_account));
        let id = faucet_account.borrow().id();

        let data_store = FaucetDataStore::new(
            faucet_account.clone(),
            account_seed,
            root_block_header,
            root_chain_mmr,
        );
        let authenticator = BasicAuthenticator::<StdRng>::new(&[(
            secret.public_key().into(),
            AuthSecretKey::RpoFalcon512(secret),
        )]);
        let executor = TransactionExecutor::new(data_store.clone(), Some(Rc::new(authenticator)));

        let mut rng = thread_rng();
        let coin_seed: [u64; 4] = rng.gen();
        let rng = RpoRandomCoin::new(coin_seed.map(Felt::new));

        Ok(Self { data_store, rpc_api, executor, id, rng })
    }

    /// Executes a mint transaction for the target account.
    ///
    /// Returns the executed transaction and the expected output note.
    pub fn execute_mint_transaction(
        &mut self,
        target_account_id: AccountId,
        is_private_note: bool,
        asset_amount: u64,
    ) -> Result<(ExecutedTransaction, Note), FaucetError> {
        let asset = FungibleAsset::new(self.id, asset_amount)
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

        let note_type = if is_private_note {
            NoteType::Private
        } else {
            NoteType::Public
        };

        let output_note = create_p2id_note(
            self.id,
            target_account_id,
            vec![asset.into()],
            note_type,
            Default::default(),
            &mut self.rng,
        )
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

        let transaction_args = build_transaction_arguments(&output_note, note_type, asset)?;

        let executed_tx = self
            .executor
            .execute_transaction(self.id, 0, &[], transaction_args)
            .map_err(|err| {
                FaucetError::InternalServerError(format!("Failed to execute transaction: {}", err))
            })?;

        Ok((executed_tx, output_note))
    }

    /// Proves and submits the executed transaction to the node.
    pub async fn prove_and_submit_transaction(
        &mut self,
        executed_tx: ExecutedTransaction,
    ) -> Result<u32, FaucetError> {
        let transaction_prover = TransactionProver::new(ProvingOptions::default());

        let delta = executed_tx.account_delta().clone();

        let proven_transaction =
            transaction_prover.prove_transaction(executed_tx).map_err(|err| {
                FaucetError::InternalServerError(format!("Failed to prove transaction: {}", err))
            })?;

        let request = SubmitProvenTransactionRequest {
            transaction: proven_transaction.to_bytes(),
        };

        let response = self
            .rpc_api
            .submit_proven_transaction(request)
            .await
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

        self.data_store.update_faucet_account(&delta).map_err(|err| {
            FaucetError::InternalServerError(format!("Failed to update account: {}", err))
        })?;

        Ok(response.into_inner().block_height)
    }

    pub fn get_faucet_id(&self) -> AccountId {
        self.id
    }
}

#[derive(Clone)]
pub struct FaucetDataStore {
    faucet_account: Rc<RefCell<Account>>,
    seed: Word,
    block_header: BlockHeader,
    chain_mmr: ChainMmr,
}

// FAUCET DATA STORE
// ================================================================================================

impl FaucetDataStore {
    pub fn new(
        faucet_account: Rc<RefCell<Account>>,
        seed: Word,
        root_block_header: BlockHeader,
        root_chain_mmr: ChainMmr,
    ) -> Self {
        Self {
            faucet_account,
            seed,
            block_header: root_block_header,
            chain_mmr: root_chain_mmr,
        }
    }

    /// Updates the stored faucet account with the provided delta.
    fn update_faucet_account(&mut self, delta: &AccountDelta) -> Result<(), FaucetError> {
        self.faucet_account
            .borrow_mut()
            .apply_delta(delta)
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))
    }
}

impl DataStore for FaucetDataStore {
    fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        _block_ref: u32,
        _notes: &[NoteId],
    ) -> Result<TransactionInputs, DataStoreError> {
        let account = self.faucet_account.borrow();
        if account_id != account.id() {
            return Err(DataStoreError::AccountNotFound(account_id));
        }

        let empty_input_notes =
            InputNotes::new(Vec::new()).map_err(DataStoreError::InvalidTransactionInput)?;

        TransactionInputs::new(
            account.clone(),
            account.is_new().then_some(self.seed),
            self.block_header,
            self.chain_mmr.clone(),
            empty_input_notes,
        )
        .map_err(DataStoreError::InvalidTransactionInput)
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Builds a new faucet account with the provided configuration.
///
/// Returns the created account, its seed, and the secret key used to sign transactions.
fn build_account(config: FaucetConfig) -> Result<(Account, Word, SecretKey), FaucetError> {
    let token_symbol = TokenSymbol::new(config.token_symbol.as_str())
        .map_err(|err| FaucetError::AccountCreationError(err.to_string()))?;

    let seed: [u8; 32] = [0; 32];

    // Instantiate keypair and authscheme
    let mut rng = ChaCha20Rng::from_seed(seed);
    let secret = SecretKey::with_rng(&mut rng);
    let auth_scheme = AuthScheme::RpoFalcon512 { pub_key: secret.public_key() };

    let (faucet_account, account_seed) = create_basic_fungible_faucet(
        seed,
        token_symbol,
        config.decimals,
        Felt::try_from(config.max_supply)
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?,
        AccountStorageMode::Private,
        auth_scheme,
    )
    .map_err(|err| FaucetError::AccountCreationError(err.to_string()))?;

    Ok((faucet_account, account_seed, secret))
}

/// Initializes the faucet client by connecting to the node and fetching the root block header.
pub async fn initialize_faucet_client(
    config: FaucetConfig,
) -> Result<(ApiClient<Channel>, BlockHeader, ChainMmr), FaucetError> {
    let endpoint = tonic::transport::Endpoint::try_from(config.node_url.clone())
        .map_err(|_| FaucetError::InternalServerError("Failed to connect to node.".to_string()))?
        .timeout(Duration::from_millis(config.timeout_ms));

    let mut rpc_api = ApiClient::connect(endpoint)
        .await
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

    let request = GetBlockHeaderByNumberRequest {
        block_num: Some(0),
        include_mmr_proof: Some(true),
    };
    let response = rpc_api.get_block_header_by_number(request).await.map_err(|err| {
        FaucetError::InternalServerError(format!("Failed to get block header: {}", err))
    })?;
    let root_block_header = response.into_inner().block_header.unwrap();

    let root_block_header: BlockHeader = root_block_header.try_into().map_err(|err| {
        FaucetError::InternalServerError(format!("Failed to parse block header: {}", err))
    })?;

    let root_chain_mmr = ChainMmr::new(
        PartialMmr::from_peaks(
            MmrPeaks::new(0, Vec::new()).expect("Empty MmrPeak should be valid"),
        ),
        Vec::new(),
    )
    .expect("Empty ChainMmr should be valid");

    Ok((rpc_api, root_block_header, root_chain_mmr))
}

/// Builds transaction arguments for the mint transaction.
fn build_transaction_arguments(
    output_note: &Note,
    note_type: NoteType,
    asset: FungibleAsset,
) -> Result<TransactionArgs, FaucetError> {
    let recipient = output_note
        .recipient()
        .digest()
        .iter()
        .map(|x| x.as_int().to_string())
        .collect::<Vec<_>>()
        .join(".");

    let tag = output_note.metadata().tag().inner();
    let aux = output_note.metadata().aux().inner();
    let execution_hint = output_note.metadata().execution_hint().into();

    let script = &DISTRIBUTE_FUNGIBLE_ASSET_SCRIPT
        .replace("{recipient}", &recipient)
        .replace("{note_type}", &Felt::new(note_type as u64).to_string())
        .replace("{aux}", &Felt::new(aux).to_string())
        .replace("{tag}", &Felt::new(tag.into()).to_string())
        .replace("{amount}", &Felt::new(asset.amount()).to_string())
        .replace("{execution_hint}", &Felt::new(execution_hint).to_string());

    let script = TransactionScript::compile(script, vec![], TransactionKernel::assembler())
        .map_err(|err| {
            FaucetError::InternalServerError(format!("Failed to compile script: {}", err))
        })?;

    let mut transaction_args = TransactionArgs::new(Some(script), None, AdviceMap::new());
    transaction_args.extend_expected_output_notes(vec![output_note.clone()]);

    Ok(transaction_args)
}
