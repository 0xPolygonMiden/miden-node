use std::time::Duration;

use miden_lib::{accounts::faucets::create_basic_fungible_faucet, AuthScheme};
use miden_node_proto::generated::{
    requests::{GetBlockHeaderByNumberRequest, SubmitProvenTransactionRequest},
    rpc::api_client::ApiClient,
};
use miden_objects::{
    accounts::{Account, AccountId, AccountStorageType},
    assembly::ModuleAst,
    assets::TokenSymbol,
    crypto::{
        dsa::rpo_falcon512::SecretKey,
        merkle::{MmrPeaks, PartialMmr},
    },
    notes::NoteId,
    transaction::{ChainMmr, ExecutedTransaction, InputNotes},
    BlockHeader, Felt, Word,
};
use miden_tx::{
    utils::Serializable, DataStore, DataStoreError, ProvingOptions, TransactionInputs,
    TransactionProver,
};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use tonic::transport::Channel;

use crate::{config::FaucetConfig, errors::FaucetError};

pub struct FaucetClient {
    data_store: FaucetDataStore,
    rpc_api: ApiClient<Channel>,
    config: FaucetConfig,
}

impl FaucetClient {
    pub async fn new(faucet_config: FaucetConfig) -> Result<Self, FaucetError> {
        let endpoint = tonic::transport::Endpoint::try_from(faucet_config.node_url.clone())
            .map_err(|_| {
                FaucetError::InternalServerError("Failed to connect to node.".to_string())
            })?
            .timeout(Duration::from_millis(faucet_config.timeout_ms));

        let mut rpc_api = ApiClient::connect(endpoint)
            .await
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

        let request = GetBlockHeaderByNumberRequest {
            block_num: Some(0),
            include_mmr_proof: Some(true),
        };
        let response = rpc_api.get_block_header_by_number(request).await.unwrap();
        let root_block_header: BlockHeader =
            response.into_inner().block_header.unwrap().try_into().unwrap();
        let root_chain_mmr = ChainMmr::new(
            PartialMmr::from_peaks(MmrPeaks::new(0, Vec::new()).unwrap()),
            Vec::new(),
        )
        .unwrap();

        let token_symbol = TokenSymbol::new(faucet_config.token_symbol.as_str())
            .map_err(|err| FaucetError::AccountCreationError(err.to_string()))?;

        let seed: [u8; 32] = [0; 32];

        // Instantiate keypair and authscheme
        let mut rng = ChaCha20Rng::from_seed(seed);
        let secret = SecretKey::with_rng(&mut rng);
        let auth_scheme = AuthScheme::RpoFalcon512 { pub_key: secret.public_key() };

        let (faucet_account, account_seed) = create_basic_fungible_faucet(
            seed,
            token_symbol,
            faucet_config.decimals,
            Felt::try_from(faucet_config.max_supply)
                .map_err(|err| FaucetError::InternalServerError(err.to_string()))?,
            AccountStorageType::OffChain,
            auth_scheme,
        )
        .unwrap();

        let data_store = FaucetDataStore::new(
            faucet_account,
            Some(account_seed),
            root_block_header,
            root_chain_mmr,
        );

        Ok(Self {
            data_store,
            rpc_api,
            config: faucet_config,
        })
    }

    pub async fn prove_and_submit_transaction(
        &mut self,
        executed_tx: ExecutedTransaction,
    ) -> Result<(), FaucetError> {
        let transaction_prover = TransactionProver::new(ProvingOptions::default());

        let proven_transaction =
            transaction_prover.prove_transaction(executed_tx).map_err(|err| {
                FaucetError::InternalServerError(format!("Failed to prove transaction: {}", err))
            })?;

        let request = SubmitProvenTransactionRequest {
            transaction: proven_transaction.to_bytes(),
        };

        self.rpc_api
            .submit_proven_transaction(request)
            .await
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

        Ok(())
    }
}

pub struct FaucetDataStore {
    faucet_account: Account,
    seed: Option<Word>,
    block_header: BlockHeader,
    chain_mmr: ChainMmr,
}

impl FaucetDataStore {
    pub fn new(
        faucet_account: Account,
        seed: Option<Word>,
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
}

impl DataStore for FaucetDataStore {
    fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        _block_ref: u32,
        _notes: &[NoteId],
    ) -> Result<TransactionInputs, DataStoreError> {
        if account_id != self.faucet_account.id() {
            return Err(DataStoreError::AccountNotFound(account_id));
        }

        let empty_input_notes =
            InputNotes::new(Vec::new()).map_err(DataStoreError::InvalidTransactionInput)?;

        TransactionInputs::new(
            self.faucet_account.clone(),
            self.seed,
            self.block_header,
            self.chain_mmr.clone(),
            empty_input_notes,
        )
        .map_err(DataStoreError::InvalidTransactionInput)
    }

    fn get_account_code(&self, account_id: AccountId) -> Result<ModuleAst, DataStoreError> {
        if account_id != self.faucet_account.id() {
            return Err(DataStoreError::AccountNotFound(account_id));
        }

        let module_ast = self.faucet_account.code().module().clone();
        Ok(module_ast)
    }
}
