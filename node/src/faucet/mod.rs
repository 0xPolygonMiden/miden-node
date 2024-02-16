use std::sync::Arc;

use actix_cors::Cors;
use actix_files::{self};
use actix_web::{http::header, post, web, App, HttpResponse, HttpServer, ResponseError};
use anyhow::Result;
use derive_more::Display;
use miden_client::{
    client::{transactions::TransactionTemplate, Client},
    config::ClientConfig,
};
use miden_objects::{accounts::AccountId, assets::FungibleAsset, utils::serde::Serializable};
use serde::Deserialize;
use tokio::sync::Mutex;

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

    // get account id from user
    let target_account_id = AccountId::from_hex(&req.account_id)
        .map_err(|e| FaucetError::BadClientData(e.to_string()))?;

    // Sync client and drop the lock before await
    let block = {
        let mut client = state.client.lock().await;
        client.sync_state().await.map_err(|e| {
            eprintln!("Failed to sync");
            FaucetError::InternalError(e.to_string())
        })?
    };

    println!("Synced to block: {block}");

    // create transaction template from data
    let template = TransactionTemplate::MintFungibleAsset {
        asset: state.asset,
        target_account_id,
    };

    // Execute, prove and submit tx
    let transaction = {
        let mut client = state.client.lock().await;
        client.new_transaction(template).map_err(|e| {
            eprintln!("Error: {}", e);
            FaucetError::InternalError(e.to_string())
        })?
    };

    println!("Transaction has been executed");

    let note_id = transaction
        .created_notes()
        .first()
        .ok_or_else(|| {
            FaucetError::InternalError("Transaction has not created a note.".to_string())
        })?
        .id();

    {
        let mut client = state.client.lock().await;
        client.send_transaction(transaction).await.map_err(|e| {
            println!("error {e}");
            FaucetError::InternalError(e.to_string())
        })?;
    }

    println!("Transaction has been proven and sent!");

    // let mut is_input_note = false;

    // for _ in 0..10 {
    //     // sync client after submitting tx to get input_note
    //     let block = state.client.lock().unwrap().sync_state().await.map_err(|e| {
    //         eprintln!("Failed to sync");
    //         FaucetError::InternalError(e.to_string())
    //     })?;

    //     println!("Synced to block: {block}");

    //     if let Ok(note) = state.client.lock().unwrap().get_input_note(note_id) {
    //         let input_note_result: Result<InputNote, _> = note.try_into();

    //         if let Ok(_input_note) = input_note_result {
    //             is_input_note = true;
    //             break;
    //         }
    //     }
    //     sleep(Duration::from_secs(1)).await;
    // }

    let note = state
        .client
        .lock()
        .await
        .get_input_note(note_id)
        .map_err(|e| FaucetError::InternalError(e.to_string()))?;

    // if is_input_note {
    let bytes = note.to_bytes();
    println!("Transaction has been turned to bytes");
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header(header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                "note.mno".to_string(),
            )],
        })
        .body(bytes))
    // } else {
    // Err(FaucetError::InternalError("Failed to return note".to_string()))
    // }
}

pub async fn serve() -> Result<()> {
    // import faucet
    let faucet = match utils::import_account_from_args() {
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
        .await
        .import_account(faucet.clone())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    println!("Faucet: {} has been loaded into client", faucet.account.id());

    let server = Arc::new(Mutex::new(
        HttpServer::new(move || {
            let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header();
            App::new()
                .app_data(web::Data::new(FaucetState {
                    client: client.clone(),
                    asset,
                }))
                .wrap(cors)
                .service(get_tokens)
                .service(
                    actix_files::Files::new("/", "faucet/src/static/").index_file("index.html"),
                )
        })
        .bind("127.0.0.1:8080")?
        .run(),
    ));

    let _ = server.lock().await;

    Ok(())
}
