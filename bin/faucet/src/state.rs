use std::sync::{Arc, Mutex};

use miden_objects::accounts::{Account, AccountId};
use tracing::info;

use crate::{client::build_account, config::FaucetConfig, errors::FaucetError};

#[derive(Clone)]
pub struct FaucetState {
    pub id: AccountId,
    pub faucet_account: Arc<Mutex<Account>>,
    pub faucet_config: FaucetConfig,
}

impl FaucetState {
    pub async fn new(config: FaucetConfig) -> Result<Self, FaucetError> {
        let (faucet_account, ..) = build_account(config.clone())?;
        let id = faucet_account.id();
        info!("Faucet initialization successful, account id: {}", faucet_account.id());
        let faucet_account = Arc::new(Mutex::new(faucet_account));

        Ok(FaucetState {
            faucet_account,
            id,
            faucet_config: config,
        })
    }
}
