use actix_cors::Cors;
use actix_files as fs;
use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use miden_client::{
    client::{self, transactions::TransactionTemplate, Client},
    config::{ClientConfig, RpcConfig, StoreConfig},
};
use miden_objects::{accounts::AccountId, assets::FungibleAsset};
use serde::Deserialize;

#[derive(Deserialize)]
struct UserId {
    account_id: String,
}

#[post("/get_tokens")]
async fn get_tokens(req: web::Json<UserId>) -> impl Responder {
    println!("Received request from account_id: {}", req.account_id);

    // let config = ClientConfig::default();
    // let client = Client::new(config)?;

    // // import faucet id from genesis generated faucet
    // let asset = FungibleAsset::new(faucet_id, 100);

    // // get account id from user
    // let account_id = AccountId::from_hex(&req.account_id)?;

    // // create transaction from data
    // let transaction = TransactionTemplate::MintFungibleAsset {
    //     asset: (),
    //     target_account_id: account_id,
    // };

    // execute, prove and submit tx
    // client.new_transaction(transaction_template)

    HttpResponse::Ok().json(format!("Token request received successfully from {}", req.account_id))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header();

        App::new()
            .wrap(cors)
            .service(get_tokens)
            .service(fs::Files::new("/", "faucet/src/static/").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
