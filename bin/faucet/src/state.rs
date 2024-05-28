use std::sync::Arc;

use async_mutex::Mutex;
use miden_objects::accounts::AccountId;
use tracing::info;

use crate::{client::FaucetClient, config::FaucetConfig, errors::FaucetError};

#[derive(Clone)]
pub struct FaucetState {
    pub id: AccountId,
    pub client: Arc<Mutex<FaucetClient>>,
    pub faucet_config: FaucetConfig,
}

impl FaucetState {
    pub async fn new(config: FaucetConfig) -> Result<Self, FaucetError> {
        let client = FaucetClient::new(config.clone()).await?;
        let id = client.get_faucet_id();
        let client = Arc::new(Mutex::new(client));
        info!("Faucet initialization successful, account id: {}", id);

        Ok(FaucetState { client, id, faucet_config: config })
    }
}
