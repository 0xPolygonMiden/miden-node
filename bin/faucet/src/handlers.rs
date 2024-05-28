use actix_web::{get, http::header, post, web, HttpResponse, Result};
use miden_client::store::InputNoteRecord;
use miden_objects::{accounts::AccountId, notes::NoteId, utils::serde::Serializable};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{errors::FaucetError, state::FaucetState};

#[derive(Deserialize)]
struct FaucetRequest {
    account_id: String,
    is_private_note: bool,
    asset_amount: u64,
}

#[derive(Serialize)]
struct FaucetMetadataReponse {
    id: String,
    asset_amount_options: Vec<u64>,
}

#[get("/get_metadata")]
pub async fn get_metadata(state: web::Data<FaucetState>) -> HttpResponse {
    let response = FaucetMetadataReponse {
        id: state.id.to_string(),
        asset_amount_options: state.config.asset_amount_options.clone(),
    };

    HttpResponse::Ok().json(response)
}

#[post("/get_tokens")]
pub async fn get_tokens(
    req: web::Json<FaucetRequest>,
    state: web::Data<FaucetState>,
) -> Result<HttpResponse> {
    info!(
        "Received a request with account_id: {}, is_private_note: {}, asset_amount: {}",
        req.account_id, req.is_private_note, req.asset_amount
    );

    // Check that the amount is in the asset amount options
    if !state.config.asset_amount_options.contains(&req.asset_amount) {
        return Err(FaucetError::BadRequest("Invalid asset amount.".to_string()).into());
    }

    let mut client = state.client.lock().await;

    // Receive and hex user account id
    let target_account_id = AccountId::from_hex(req.account_id.as_str())
        .map_err(|err| FaucetError::BadRequest(err.to_string()))?;

    // Execute transaction
    let (executed_tx, created_note) = client.execute_mint_transaction(
        target_account_id,
        req.is_private_note,
        req.asset_amount,
    )?;

    // Run transaction prover & send transaction to node
    client.prove_and_submit_transaction(executed_tx).await?;

    let note_id: NoteId = created_note.id();

    // Serialize note into bytes
    let bytes = InputNoteRecord::from(created_note).to_bytes();

    info!("A new note has been created: {}", note_id);

    // Send generated note to user
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header(header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                "note.mno".to_string(),
            )],
        })
        .append_header(("Note-Id", note_id.to_string()))
        .body(bytes))
}
