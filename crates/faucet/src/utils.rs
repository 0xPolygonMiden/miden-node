use std::{path::PathBuf, sync::Arc};

use async_mutex::Mutex;
use miden_client::{
    client::{get_random_coin, rpc::TonicRpcClient, Client},
    config::{RpcConfig, StoreConfig},
    store::{sqlite_store::SqliteStore, AuthInfo},
};
use miden_lib::{accounts::faucets::create_basic_fungible_faucet, AuthScheme};
use miden_objects::{
    accounts::{Account, AccountId, AccountStorageType},
    assets::TokenSymbol,
    crypto::{dsa::rpo_falcon512::SecretKey, rand::RpoRandomCoin},
    Felt,
};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use tracing::info;

use crate::{config::FaucetConfig, errors::FaucetError};

pub type FaucetClient = Client<TonicRpcClient, RpoRandomCoin, SqliteStore>;

#[derive(Clone)]
pub struct FaucetState {
    pub id: AccountId,
    pub asset_amount: u64,
    pub client: Arc<Mutex<FaucetClient>>,
}

/// Instatiantes the Miden faucet
pub async fn build_faucet_state(config: FaucetConfig) -> Result<FaucetState, FaucetError> {
    let mut client = build_client(config.database_filepath.clone())?;

    let faucet_account = create_fungible_faucet(
        &config.token_symbol,
        &config.decimals,
        &config.max_supply,
        &mut client,
    )?;

    // Sync client
    client.sync_state().await.map_err(FaucetError::SyncError)?;

    info!("Faucet initialization successful, account id: {}", faucet_account.id());

    Ok(FaucetState {
        id: faucet_account.id(),
        asset_amount: config.asset_amount,
        client: Arc::new(Mutex::new(client)),
    })
}

/// Instantiates the Miden client
pub fn build_client(database_filepath: PathBuf) -> Result<FaucetClient, FaucetError> {
    let database_filepath_os_string = database_filepath.into_os_string();
    let database_filepath = match database_filepath_os_string.into_string() {
        Ok(string) => string,
        Err(e) => {
            return Err(FaucetError::DatabaseError(format!(
                "Failed to read database filepath: {:?}",
                e
            )))
        },
    };

    // Setup store
    let store_config = StoreConfig {
        database_filepath: database_filepath.clone(),
    };
    let store = SqliteStore::new(store_config)
        .map_err(|err| FaucetError::DatabaseError(err.to_string()))?;

    // Setup the executor store
    let executor_store_config = StoreConfig {
        database_filepath: database_filepath.clone(),
    };
    let executor_store = SqliteStore::new(executor_store_config)
        .map_err(|err| FaucetError::DatabaseError(err.to_string()))?;

    // Setup the tonic rpc client
    let rpc_config = RpcConfig::default();
    let api = TonicRpcClient::new(&rpc_config.endpoint.to_string());

    // Setup the rng
    let rng = get_random_coin();

    info!("Successfully built client");

    // Setup the client
    Client::new(api, rng, store, executor_store).map_err(FaucetError::ClientCreationError)
}

/// Creates a Miden fungible faucet from arguments
pub fn create_fungible_faucet(
    token_symbol: &str,
    decimals: &u8,
    max_supply: &u64,
    client: &mut FaucetClient,
) -> Result<Account, FaucetError> {
    let token_symbol = TokenSymbol::new(token_symbol)
        .map_err(|err| FaucetError::AccountCreationError(err.to_string()))?;

    // Instantiate seed
    let seed: [u8; 32] = [0; 32];

    // Instantiate keypair and authscheme
    let mut rng = ChaCha20Rng::from_seed(seed);
    let secret = SecretKey::with_rng(&mut rng);
    let auth_scheme = AuthScheme::RpoFalcon512 { pub_key: secret.public_key() };

    let (account, account_seed) = create_basic_fungible_faucet(
        seed,
        token_symbol,
        *decimals,
        Felt::try_from(*max_supply)
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?,
        AccountStorageType::OffChain,
        auth_scheme,
    )
    .map_err(|err| FaucetError::AccountCreationError(err.to_string()))?;

    client
        .insert_account(&account, Some(account_seed), &AuthInfo::RpoFalcon512(secret))
        .map_err(|err| FaucetError::DatabaseError(err.to_string()))?;

    Ok(account)
}
