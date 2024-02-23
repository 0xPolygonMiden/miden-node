use std::{io, sync::Arc};

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{web, App, HttpServer};
use async_mutex::Mutex;
use clap::Parser;
use cli::Cli;
use handlers::get_tokens;
use miden_client::{
    client::{rpc::TonicRpcClient, Client},
    config::{ClientConfig, RpcConfig, StoreConfig},
    store::{data_store::SqliteDataStore, Store},
};
use miden_objects::accounts::AccountId;

mod cli;
mod errors;
mod handlers;
mod utils;

#[derive(Clone)]
pub struct FaucetState {
    id: AccountId,
    asset_amount: u64,
    client: Arc<Mutex<Client<TonicRpcClient, SqliteDataStore>>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    // Setup the data_store
    let store_config = StoreConfig::default();
    let store = Store::new(store_config).expect("Failed to instantiate store.");
    let data_store = SqliteDataStore::new(store);

    // Setup the tonic rpc client
    let rpc_config = RpcConfig::default();
    let api = TonicRpcClient::new(&rpc_config.endpoint.to_string());

    // Setup the client
    let client_config = ClientConfig::default();
    let mut client =
        Client::new(client_config, api, data_store).expect("Failed to instantiate client.");

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
    client
        .sync_state()
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::ConnectionRefused, "Failed to sync state."))?;

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
            .service(get_tokens)
            .service(Files::new("/", "src/static").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    Ok(())
}
