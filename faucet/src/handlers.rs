use actix_web::{get, http::header, post, web, HttpResponse, Result};
use miden_client::client::transactions::TransactionTemplate;
use miden_objects::{
    accounts::AccountId, assets::FungibleAsset, notes::NoteId, utils::serde::Serializable,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{errors::FaucetError, FaucetState};

#[derive(Deserialize)]
struct FaucetRequest {
    account_id: String,
}

#[derive(Serialize)]
struct FaucetMetadataReponse {
    id: String,
    asset_amount: u64,
}

#[get("/get_metadata")]
pub async fn get_metadata(state: web::Data<FaucetState>) -> HttpResponse {
    let response = FaucetMetadataReponse {
        id: state.id.to_string(),
        asset_amount: state.asset_amount,
    };

    HttpResponse::Ok().json(response)
}

#[post("/get_tokens")]
pub async fn get_tokens(
    req: web::Json<FaucetRequest>,
    state: web::Data<FaucetState>,
) -> Result<HttpResponse> {
    info!("Received a request with account_id: {}", req.account_id);

    let client = state.client.clone();

    // Receive and hex user account id
    let target_account_id = AccountId::from_hex(req.account_id.as_str())
        .map_err(|err| FaucetError::BadRequest(err.to_string()))?;

    // Instantiate asset
    let asset =
        FungibleAsset::new(state.id, state.asset_amount).expect("Failed to instantiate asset.");

    // Instantiate transaction template
    let tx_template = TransactionTemplate::MintFungibleAsset {
        asset,
        target_account_id,
    };

    // Run transaction executor & execute transaction
    let tx_result = client
        .lock()
        .await
        .new_transaction(tx_template)
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

    // Get note id
    let note_id: NoteId = tx_result
        .created_notes()
        .first()
        .ok_or_else(|| {
            FaucetError::InternalServerError("Failed to access generated note.".to_string())
        })?
        .id();

    // Run transaction prover & send transaction to node
    {
        let mut client_guard = client.lock().await;
        client_guard
            .send_transaction(tx_result)
            .await
            .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;
    }

    // Get note from client store
    let input_note = state
        .client
        .clone()
        .lock()
        .await
        .get_input_note(note_id)
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))?;

    // Serialize note for transport
    let bytes = input_note.to_bytes();

    // Send generated note to user
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header(header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                "note.mno".to_string(),
            )],
        })
        .body(bytes))
}
