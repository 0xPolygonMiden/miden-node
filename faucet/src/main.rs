use std::sync::{Arc, Mutex};

use actix_cors::Cors;
use actix_files;
use actix_web::{post, web, App, HttpResponse, HttpServer, ResponseError};
use derive_more::Display;
use miden_client::{
    client::{transactions::TransactionTemplate, Client},
    config::ClientConfig,
};
use miden_objects::{accounts::AccountId, assets::FungibleAsset};
use serde::Deserialize;
use utils::import_account_from_args;

mod utils;

#[derive(Debug, Display)]
enum FaucetError {
    #[display(fmt = "Internal server error")]
    InternalError(String),

    #[display(fmt = "Bad client request data")]
    BadClientData(String),
}

impl ResponseError for FaucetError {}

#[derive(Deserialize)]
struct UserId {
    account_id: String,
}

struct FaucetState {
    client: Arc<Mutex<Client>>,
    asset: FungibleAsset,
}

#[post("/get_tokens")]
async fn get_tokens(
    state: web::Data<FaucetState>,
    req: web::Json<UserId>,
) -> Result<HttpResponse, FaucetError> {
    println!("Received request from account_id: {}", req.account_id);

    // sync client
    let block = state.client.lock().unwrap().sync_state().await.map_err(|e| {
        eprintln!("Failed to sync");
        FaucetError::InternalError(e.to_string())
    })?;

    println!("synced {block}");

    // get account id from user
    let target_account_id = AccountId::from_hex(&req.account_id)
        .map_err(|e| FaucetError::BadClientData(e.to_string()))?;

    // create transaction template from data
    let template = TransactionTemplate::MintFungibleAsset {
        asset: state.asset,
        target_account_id,
    };

    // execute, prove and submit tx
    let transaction = state.client.lock().unwrap().new_transaction(template).map_err(|e| {
        eprintln!("Error: {}", e.to_string());
        FaucetError::InternalError(e.to_string())
    })?;

    println!("Transaction has been executed");

    state.client.lock().unwrap().send_transaction(transaction).await.map_err(|e| {
        eprintln!("Error: {}", e.to_string());
        FaucetError::InternalError(e.to_string())
    })?;

    println!("Transaction has been proven and sent!");

    Ok(HttpResponse::Ok()
        .json(format!("Token request received successfully from {}", req.account_id)))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // import faucet
    let faucet = match import_account_from_args() {
        Ok(account_data) => account_data,
        Err(e) => panic!("Failed importing faucet account: {e}"),
    };

    // init asset
    let asset = FungibleAsset::new(faucet.account.id(), 100)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // init client & Arc<Mutex<Client>> to enable safe thread passing and mutability
    let config = ClientConfig::default();
    let client = Arc::new(Mutex::new(
        Client::new(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?,
    ));

    // load faucet into client
    client
        .lock()
        .unwrap()
        .import_account(faucet)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    HttpServer::new(move || {
        let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header();

        App::new()
            .app_data(web::Data::new(FaucetState {
                client: client.clone(),
                asset,
            }))
            .wrap(cors)
            .service(get_tokens)
            .service(actix_files::Files::new("/", "faucet/src/static/").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
