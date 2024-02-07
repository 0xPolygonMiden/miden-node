use std::{env, fs, path::PathBuf, str::FromStr};

use actix_cors::Cors;
use actix_files;
use actix_web::{post, web, App, HttpResponse, HttpServer, ResponseError};
use derive_more::Display;
use miden_client::{
    client::{transactions::TransactionTemplate, Client},
    config::ClientConfig,
};
use miden_objects::{
    accounts::{AccountData, AccountId},
    assets::FungibleAsset,
    utils::serde::Deserializable,
};
use serde::Deserialize;

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

struct AccountDataWrapper {
    account_data: AccountData,
}

#[post("/get_tokens")]
async fn get_tokens(
    account_data_wrapper: web::Data<AccountDataWrapper>,
    req: web::Json<UserId>,
) -> Result<HttpResponse, FaucetError> {
    println!("Received request from account_id: {}", req.account_id);

    let account_data = account_data_wrapper.account_data.clone();

    let faucet_id = account_data.account.id();

    let config = ClientConfig::default();
    let mut client = Client::new(config).map_err(|e| FaucetError::BadClientData(e.to_string()))?;

    // import faucet into client
    client
        .import_account(account_data)
        .map_err(|e| FaucetError::InternalError(e.to_string()))?;

    println!("Faucet account has been imported");

    // sync client
    let block = client
        .sync_state()
        .await
        .map_err(|e| FaucetError::InternalError(e.to_string()))?;

    println!("synced {block}");

    // get faucet_id and create asset
    let asset = FungibleAsset::new(faucet_id, 100).map_err(|e| {
        eprintln!("Error: {}", e.to_string());
        FaucetError::InternalError(e.to_string())
    })?;

    // get account id from user
    let target_account_id = AccountId::from_hex(&req.account_id)
        .map_err(|e| FaucetError::BadClientData(e.to_string()))?;

    // create transaction from data
    let template = TransactionTemplate::MintFungibleAsset {
        asset,
        target_account_id,
    };

    println!("faucet id: {:?}", faucet_id);
    println!("account id: {:?}", target_account_id);
    println!("asset id: {:?}", asset);

    println!("transaction has been built!");

    // execute, prove and submit tx
    let transaction = client.new_transaction(template).map_err(|e| {
        eprintln!("Error: {}", e.to_string());
        FaucetError::InternalError(e.to_string())
    })?;

    println!("Transaction has been executed");

    client.send_transaction(transaction).await.map_err(|e| {
        eprintln!("Error: {}", e.to_string());
        FaucetError::InternalError(e.to_string())
    })?;

    println!("Transaction has been proven and sent!");

    Ok(HttpResponse::Ok()
        .json(format!("Token request received successfully from {}", req.account_id)))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path_string = args
        .get(1)
        .expect("A faucet .mac file should be provided to initialize the server");
    let path = PathBuf::from_str(&path_string).expect("Invalid path.");
    let account_data_file_contents = fs::read(path).expect("Failed to read file");
    let account_data = AccountData::read_from_bytes(&account_data_file_contents)
        .expect("Failed to deserialize account");

    HttpServer::new(move || {
        let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header();

        App::new()
            .app_data(web::Data::new(AccountDataWrapper { account_data: account_data.clone() })) // Pass faucet_id to the app state
            .wrap(cors)
            .service(get_tokens)
            .service(actix_files::Files::new("/", "faucet/src/static/").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
