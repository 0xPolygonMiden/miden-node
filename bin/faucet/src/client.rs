use std::{cell::RefCell, time::Duration};

use miden_lib::{accounts::faucets::create_basic_fungible_faucet, AuthScheme};
use miden_node_proto::generated::{
    requests::{GetBlockHeaderByNumberRequest, SubmitProvenTransactionRequest},
    rpc::api_client::ApiClient,
};
use miden_objects::{
    accounts::{Account, AccountDelta, AccountId, AccountStorageType, AuthSecretKey},
    assembly::ModuleAst,
    assets::TokenSymbol,
    crypto::{
        dsa::rpo_falcon512::{self, Polynomial, SecretKey},
        merkle::{MmrPeaks, PartialMmr},
    },
    notes::NoteId,
    transaction::{ChainMmr, ExecutedTransaction, InputNotes},
    BlockHeader, Felt, Word,
};
use miden_tx::{
    utils::Serializable, AuthenticationError, DataStore, DataStoreError, ProvingOptions,
    TransactionAuthenticator, TransactionInputs, TransactionProver,
};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use tonic::transport::Channel;

use crate::{config::FaucetConfig, errors::FaucetError};

pub struct FaucetClient {
    data_store: FaucetDataStore,
    authenticator: FaucetAuthenticator,
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

        let authenticator = FaucetAuthenticator::new(secret);

        Ok(Self {
            data_store,
            rpc_api,
            config: faucet_config,
            authenticator,
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

struct FaucetAuthenticator {
    faucet_secret_key: AuthSecretKey,
    rng: RefCell<ChaCha20Rng>,
}

impl FaucetAuthenticator {
    pub fn new(faucet_secret_key: SecretKey) -> Self {
        Self {
            faucet_secret_key: AuthSecretKey::RpoFalcon512(faucet_secret_key),
            rng: RefCell::new(ChaCha20Rng::from_entropy()),
        }
    }
}

impl TransactionAuthenticator for FaucetAuthenticator {
    fn get_signature(
        &self,
        _pub_key: Word,
        message: Word,
        _account_delta: &AccountDelta,
    ) -> Result<Vec<Felt>, AuthenticationError> {
        let mut rng = self.rng.borrow_mut();
        let AuthSecretKey::RpoFalcon512(k) = &self.faucet_secret_key;
        get_falcon_signature(k, message, &mut rng)
    }
}

// HELPER FUNCTIONS
// ================================================================================================

// TODO: Remove the falcon signature function once it's available on base and made public

/// Retrieves a falcon signature over a message.
/// Gets as input a [Word] containing a secret key, and a [Word] representing a message and
/// outputs a vector of values to be pushed onto the advice stack.
/// The values are the ones required for a Falcon signature verification inside the VM and they are:
///
/// 1. The nonce represented as 8 field elements.
/// 2. The expanded public key represented as the coefficients of a polynomial of degree < 512.
/// 3. The signature represented as the coefficients of a polynomial of degree < 512.
/// 4. The product of the above two polynomials in the ring of polynomials with coefficients
/// in the Miden field.
///
/// # Errors
/// Will return an error if either:
/// - The secret key is malformed due to either incorrect length or failed decoding.
/// - The signature generation failed.
///
/// TODO: once this gets made public in miden base, remve this implementation and use the one from
/// base
fn get_falcon_signature(
    key: &rpo_falcon512::SecretKey,
    message: Word,
    rng: &mut ChaCha20Rng,
) -> Result<Vec<Felt>, AuthenticationError> {
    // Generate the signature
    let sig = key.sign_with_rng(message, rng);
    // The signature is composed of a nonce and a polynomial s2
    // The nonce is represented as 8 field elements.
    let nonce = sig.nonce();
    // We convert the signature to a polynomial
    let s2 = sig.sig_poly();
    // We also need in the VM the expanded key corresponding to the public key the was provided
    // via the operand stack
    let h = key.compute_pub_key_poly().0;
    // Lastly, for the probabilistic product routine that is part of the verification procedure,
    // we need to compute the product of the expanded key and the signature polynomial in
    // the ring of polynomials with coefficients in the Miden field.
    let pi = Polynomial::mul_modulo_p(&h, s2);
    // We now push the nonce, the expanded key, the signature polynomial, and the product of the
    // expanded key and the signature polynomial to the advice stack.
    let mut result: Vec<Felt> = nonce.to_elements().to_vec();

    result.extend(h.coefficients.iter().map(|a| Felt::from(a.value() as u32)));
    result.extend(s2.coefficients.iter().map(|a| Felt::from(a.value() as u32)));
    result.extend(pi.iter().map(|a| Felt::new(*a)));
    result.reverse();
    Ok(result)
}
