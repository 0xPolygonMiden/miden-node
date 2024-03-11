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
use miden_client::client::Client;
use miden_client::config::{RpcConfig, StoreConfig};
use miden_client::store::data_store::SqliteDataStore;
use miden_client::{client::rpc::TonicRpcClient, store::sqlite_store::SqliteStore};
use miden_objects::accounts::AccountId;
use std::io;
use std::sync::Arc;

mod cli;
mod errors;
mod handlers;
mod utils;

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

    // Setup store
    let store_config = StoreConfig::default();
    let store = SqliteStore::new(store_config).expect("Failed to instantiate store.");

    // Setup the data_store
    let data_store_store_config = StoreConfig::default();
    let data_store_store =
        SqliteStore::new(data_store_store_config).expect("Failed to instantiat datastore store");
    let data_store = SqliteDataStore::new(data_store_store);

    // Setup the tonic rpc client
    let rpc_config = RpcConfig::default();
    let api = TonicRpcClient::new(&rpc_config.endpoint.to_string());

    // Setup the client
    let mut client = Client::new(api, store, data_store).expect("Failed to instantiate client.");

    // // Instantiate and load config
    // let client_config = ClientConfig::default();

    let amount: u64;

    // Create the faucet account
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

    println!("✅ Faucet setup successful, account id: {}", faucet_account.id());

    println!("🚀 Starting server on: http://127.0.0.1:8080");

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
