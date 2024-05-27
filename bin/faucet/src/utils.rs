use miden_objects::accounts::AccountId;
use tracing::info;

use crate::{client::FaucetClient, config::FaucetConfig, errors::FaucetError};

#[derive(Clone)]
pub struct FaucetState {
    pub id: AccountId,
    pub faucet_config: FaucetConfig,
}

/// Instatiantes the Miden faucet
pub async fn build_faucet_state(config: FaucetConfig) -> Result<FaucetState, FaucetError> {
    let (faucet_account, ..) = FaucetClient::build_account(config.clone())?;

    info!("Faucet initialization successful, account id: {}", faucet_account.id());

    Ok(FaucetState {
        id: faucet_account.id(),
        faucet_config: config,
    })
}
