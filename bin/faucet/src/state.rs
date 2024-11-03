use std::{collections::HashMap, sync::Arc};

use miden_objects::accounts::AccountId;
use static_files::Resource;
use tokio::sync::Mutex;
use tracing::info;

use crate::{client::FaucetClient, config::FaucetConfig, static_resources, COMPONENT};

// FAUCET STATE
// ================================================================================================

/// Stores the client and additional information needed to handle requests.
///
/// The state is passed to every mint transaction request so the client is
/// shared between handler threads.
#[derive(Clone)]
pub struct FaucetState {
    pub id: AccountId,
    pub client: Arc<Mutex<FaucetClient>>,
    pub config: FaucetConfig,
    pub static_files: Arc<HashMap<&'static str, Resource>>,
}

impl FaucetState {
    pub async fn new(config: FaucetConfig) -> anyhow::Result<Self> {
        let client = FaucetClient::new(&config).await?;
        let id = client.get_faucet_id();
        let client = Arc::new(Mutex::new(client));
        let static_files = Arc::new(static_resources::generate());

        info!(target: COMPONENT, account_id = %id, "Faucet initialization successful");

        Ok(FaucetState { client, id, config, static_files })
    }
}
