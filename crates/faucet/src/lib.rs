use std::{io, path::Path, sync::Arc};

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{
    middleware::{DefaultHeaders, Logger},
    web, App, HttpServer,
};
use async_mutex::Mutex;
use handlers::{get_metadata, get_tokens};
use miden_client::{
    client::{rpc::TonicRpcClient, Client},
    store::sqlite_store::SqliteStore,
};
use miden_node_utils::config::load_config;
use miden_objects::{accounts::AccountId, crypto::rand::RpoRandomCoin};
use tracing::info;

use crate::config::FaucetTopLevelConfig;

pub mod config;
mod errors;
mod handlers;
mod utils;

pub type FaucetClient = Client<TonicRpcClient, RpoRandomCoin, SqliteStore>;

#[derive(Clone)]
pub struct FaucetState {
    id: AccountId,
    asset_amount: u64,
    client: Arc<Mutex<FaucetClient>>,
}

pub async fn start_faucet(config_filepath: &Path) -> std::io::Result<()> {
    let config: FaucetTopLevelConfig = load_config(config_filepath).extract().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to load configuration file: {}", e),
        )
    })?;
    let config = config.faucet;
    let mut client = utils::build_client(config.database_filepath.clone());
    let faucet_account = utils::create_fungible_faucet(
        &config.token_symbol,
        &config.decimals,
        &config.max_supply,
        &mut client,
    )
    .map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to create faucet account: {}", err),
        )
    })?;

    // Sync client
    client.sync_state().await.map_err(|e| {
        io::Error::new(io::ErrorKind::NotConnected, format!("Failed to sync state: {e:?}"))
    })?;

    info!("✅ Faucet setup successful, account id: {}", faucet_account.id());

    info!("🚀 Starting server on: {}", config.as_url());

    // Instantiate faucet state
    let faucet_state = FaucetState {
        id: faucet_account.id(),
        asset_amount: config.asset_amount,
        client: Arc::new(Mutex::new(client)),
    };

    HttpServer::new(move || {
        let cors = Cors::default().allow_any_origin().allow_any_method();
        App::new()
            .app_data(web::Data::new(faucet_state.clone()))
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("Cache-Control", "no-cache")))
            .service(get_metadata)
            .service(get_tokens)
            .service(
                Files::new("/", "crates/faucet/src/static")
                    .use_etag(false)
                    .use_last_modified(false)
                    .index_file("index.html"),
            )
    })
    .bind((config.endpoint.host, config.endpoint.port))?
    .run()
    .await?;

    Ok(())
}
