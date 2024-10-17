use std::{
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{anyhow, Context};
use miden_lib::{
    accounts::faucets::create_basic_fungible_faucet, notes::create_p2id_note,
    transaction::TransactionKernel, AuthScheme,
};
use miden_node_proto::generated::{
    requests::{
        GetAccountDetailsRequest, GetBlockHeaderByNumberRequest, SubmitProvenTransactionRequest,
    },
    rpc::api_client::ApiClient,
};
use miden_objects::{
    accounts::{Account, AccountId, AccountStorageMode, AuthSecretKey},
    assets::{FungibleAsset, TokenSymbol},
    crypto::{
        dsa::rpo_falcon512::SecretKey,
        merkle::{MmrPeaks, PartialMmr},
        rand::RpoRandomCoin,
    },
    notes::{Note, NoteType},
    transaction::{ChainMmr, ExecutedTransaction, TransactionArgs, TransactionScript},
    utils::Deserializable,
    vm::AdviceMap,
    BlockHeader, Felt, Word,
};
use miden_tx::{
    auth::BasicAuthenticator, utils::Serializable, LocalTransactionProver, ProvingOptions,
    TransactionExecutor, TransactionProver,
};
use rand::{rngs::StdRng, thread_rng, Rng};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use tonic::transport::Channel;

use crate::{
    config::FaucetConfig,
    errors::{ErrorHelper, HandlerError},
    store::FaucetDataStore,
};

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

impl FaucetClient {
    /// Creates a new faucet client.
    pub async fn new(config: &FaucetConfig) -> anyhow::Result<Self> {
        let (rpc_api, root_block_header, root_chain_mmr) = initialize_faucet_client(config).await?;
        let init_seed: [u8; 32] = [0; 32];
        let (auth_scheme, authenticator) = init_authenticator(init_seed, &config.secret_key_path)
            .context("Failed to initialize authentication scheme")?;

        let (faucet_account, account_seed) = build_account(config, init_seed, auth_scheme)?;
        let id = faucet_account.id();

        let data_store = FaucetDataStore::new(
            Arc::new(RwLock::new(faucet_account)),
            account_seed,
            root_block_header,
            root_chain_mmr,
        );

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
    ) -> Result<(ExecutedTransaction, Note), HandlerError> {
        let asset =
            FungibleAsset::new(self.id, asset_amount).or_fail("Failed to create fungible asset")?;

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
        .or_fail("Failed to create P2ID note")?;

        let transaction_args = build_transaction_arguments(&output_note, note_type, asset)?;

        let executed_tx = self
            .executor
            .execute_transaction(self.id, 0, &[], transaction_args)
            .or_fail("Failed to execute transaction")?;

        Ok((executed_tx, output_note))
    }

    /// Proves and submits the executed transaction to the node.
    pub async fn prove_and_submit_transaction(
        &mut self,
        executed_tx: ExecutedTransaction,
    ) -> Result<u32, HandlerError> {
        // Prepare request with proven transaction.
        // This is needed to be in a separated code block in order to release reference to avoid
        // borrow checker error.
        let request = {
            let transaction_prover = LocalTransactionProver::new(ProvingOptions::default());

            let proven_transaction =
                transaction_prover.prove(executed_tx).or_fail("Failed to prove transaction")?;

            SubmitProvenTransactionRequest {
                transaction: proven_transaction.to_bytes(),
            }
        };

        let response = self
            .rpc_api
            .submit_proven_transaction(request)
            .await
            .or_fail("Failed to submit proven transaction")?;

        Ok(response.into_inner().block_height)
    }

    /// Requests faucet account state from the node.
    ///
    /// The account is expected to be public, otherwise, the error is returned.
    pub async fn request_account_state(&mut self) -> Result<(Account, u32), HandlerError> {
        let account_info = self
            .rpc_api
            .get_account_details(GetAccountDetailsRequest { account_id: Some(self.id.into()) })
            .await
            .or_fail("Failed to get faucet account state")?
            .into_inner()
            .details
            .or_fail("Account info field is empty")?;

        let faucet_account_state_bytes =
            account_info.details.or_fail("Account details field is empty")?;
        let faucet_account =
            Account::read_from_bytes(&faucet_account_state_bytes).map_err(|err| {
                HandlerError::InternalServerError(format!(
                    "Failed to deserialize faucet account: {err}"
                ))
            })?;
        let block_num = account_info.summary.or_fail("Account summary field is empty")?.block_num;

        Ok((faucet_account, block_num))
    }

    /// Returns a reference to the data store.
    pub fn data_store(&self) -> &FaucetDataStore {
        &self.data_store
    }

    /// Returns the id of the faucet account.
    pub fn get_faucet_id(&self) -> AccountId {
        self.id
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Initializes the keypair used to sign transactions.
///
/// If the secret key file exists, it is read from the file. Otherwise, a new key is generated and
/// written to the file.
fn init_authenticator(
    init_seed: [u8; 32],
    secret_key_path: impl AsRef<Path>,
) -> anyhow::Result<(AuthScheme, BasicAuthenticator<StdRng>)> {
    // Load secret key from file or generate new one
    let secret = if secret_key_path.as_ref().exists() {
        SecretKey::read_from_bytes(
            &std::fs::read(secret_key_path).context("Failed to read secret key from file")?,
        )
        .map_err(|err| anyhow!("Failed to deserialize secret key: {err}"))?
    } else {
        let mut rng = ChaCha20Rng::from_seed(init_seed);
        let secret = SecretKey::with_rng(&mut rng);
        std::fs::write(secret_key_path, secret.to_bytes())
            .context("Failed to write secret key to file")?;

        secret
    };

    let auth_scheme = AuthScheme::RpoFalcon512 { pub_key: secret.public_key() };

    let authenticator = BasicAuthenticator::<StdRng>::new(&[(
        secret.public_key().into(),
        AuthSecretKey::RpoFalcon512(secret),
    )]);

    Ok((auth_scheme, authenticator))
}

/// Builds a new faucet account with the provided configuration.
///
/// Returns the created account, its seed, and the secret key used to sign transactions.
fn build_account(
    config: &FaucetConfig,
    init_seed: [u8; 32],
    auth_scheme: AuthScheme,
) -> anyhow::Result<(Account, Word)> {
    let token_symbol = TokenSymbol::new(config.token_symbol.as_str())
        .context("Failed to parse token symbol from configuration file")?;

    let (faucet_account, account_seed) = create_basic_fungible_faucet(
        init_seed,
        token_symbol,
        config.decimals,
        Felt::try_from(config.max_supply)
            .map_err(|err| anyhow!("Error converting max supply to Felt: {err}"))?,
        AccountStorageMode::Public,
        auth_scheme,
    )
    .context("Failed to create basic fungible faucet account")?;

    Ok((faucet_account, account_seed))
}

/// Initializes the faucet client by connecting to the node and fetching the root block header.
pub async fn initialize_faucet_client(
    config: &FaucetConfig,
) -> anyhow::Result<(ApiClient<Channel>, BlockHeader, ChainMmr)> {
    let endpoint = tonic::transport::Endpoint::try_from(config.node_url.clone())
        .context("Failed to parse node URL from configuration file")?
        .timeout(Duration::from_millis(config.timeout_ms));

    let mut rpc_api =
        ApiClient::connect(endpoint).await.context("Failed to connect to the node")?;

    let request = GetBlockHeaderByNumberRequest {
        block_num: Some(0),
        include_mmr_proof: None,
    };
    let response = rpc_api
        .get_block_header_by_number(request)
        .await
        .context("Failed to get block header")?;
    let root_block_header = response
        .into_inner()
        .block_header
        .context("Missing root block header in response")?;

    let root_block_header = root_block_header.try_into().context("Failed to parse block header")?;

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
) -> Result<TransactionArgs, HandlerError> {
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
        .or_fail("Failed to compile script")?;

    let mut transaction_args = TransactionArgs::new(Some(script), None, AdviceMap::new());
    transaction_args.extend_expected_output_notes(vec![output_note.clone()]);

    Ok(transaction_args)
}
