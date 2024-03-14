use std::{io, sync::Arc};

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{
    middleware::{DefaultHeaders, Logger},
    web, App, HttpServer,
};
use async_mutex::Mutex;
use clap::Parser;
use cli::Cli;
use handlers::{faucet_id, get_tokens};
use miden_client::{
    client::{rpc::TonicRpcClient, Client},
    store::{data_store::SqliteDataStore, sqlite_store::SqliteStore},
};
use miden_objects::accounts::AccountId;
use miden_node_utils::config::load_config;

use config::FaucetTopLevelConfig;

mod cli;
mod errors;
mod handlers;
mod utils;
mod config;

#[derive(Clone)]
pub struct FaucetState {
    id: AccountId,
    asset_amount: u64,
    client: Arc<Mutex<Client<TonicRpcClient, SqliteStore, SqliteDataStore>>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Load config
    let config: FaucetTopLevelConfig = load_config(cli.config.as_path()).extract()
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to load configuration file: {}", e)))?;

    // Instantiate Miden client
    let mut client = utils::build_client();

    let amount: u64;

    // Create faucet account
    let faucet_account = match &cli.command {
        cli::Command::Init {
            token_symbol,
            decimals,
            max_supply,
            asset_amount,
        } => {
            amount = *asset_amount;
            utils::create_fungible_faucet(token_symbol, decimals, max_supply, &mut client)
        },
        cli::Command::Import {
            faucet_path,
            asset_amount,
        } => {
            amount = *asset_amount;
            utils::import_fungible_faucet(faucet_path, &mut client)
        },
    }
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to create faucet account."))?;

    // Sync client
    client.sync_state().await.map_err(|e| {
        io::Error::new(io::ErrorKind::ConnectionRefused, format!("Failed to sync state: {e:?}"))
    })?;

    println!("{}", config.faucet);

    println!("✅ Faucet setup successful, account id: {}", faucet_account.id());

    println!("🚀 Starting server on: {}", config.faucet.as_url());

    // Instantiate faucet state
    let faucet_state = FaucetState {
        id: faucet_account.id(),
        asset_amount: amount,
        client: Arc::new(Mutex::new(client)),
    };

    HttpServer::new(move || {
        let cors = Cors::default().allow_any_origin().allowed_methods(vec!["GET"]);
        App::new()
            .app_data(web::Data::new(faucet_state.clone()))
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("Cache-Control", "no-cache")))
            .service(faucet_id)
            .service(get_tokens)
            .service(
                Files::new("/", "faucet/src/static")
                    .use_etag(false)
                    .use_last_modified(false)
                    .index_file("index.html"),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    Ok(())
}
