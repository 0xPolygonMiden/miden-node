use axum::{
    extract::{Path, State},
    http::{Response, StatusCode},
    response::IntoResponse,
    Json,
};
use http::header;
use http_body_util::Full;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteDetails, NoteExecutionMode, NoteFile, NoteId, NoteTag},
    utils::serde::Serializable,
};
use serde::{Deserialize, Serialize};
use tonic::body;
use tracing::info;

use crate::{errors::FaucetError, state::FaucetState, COMPONENT};

#[derive(Deserialize)]
pub struct FaucetRequest {
    account_id: String,
    is_private_note: bool,
    asset_amount: u64,
}

#[derive(Serialize)]
pub struct FaucetMetadataReponse {
    id: String,
    asset_amount_options: Vec<u64>,
}

pub async fn get_metadata(
    State(state): State<FaucetState>,
) -> (StatusCode, Json<FaucetMetadataReponse>) {
    let response = FaucetMetadataReponse {
        id: state.id.to_string(),
        asset_amount_options: state.config.asset_amount_options.clone(),
    };

    (StatusCode::OK, Json(response))
}

pub async fn get_tokens(
    State(state): State<FaucetState>,
    Json(req): Json<FaucetRequest>,
) -> Result<impl IntoResponse, FaucetError> {
    info!(
        "Received a request with account_id: {}, is_private_note: {}, asset_amount: {}",
        req.account_id, req.is_private_note, req.asset_amount
    );

    // Check that the amount is in the asset amount options
    if !state.config.asset_amount_options.contains(&req.asset_amount) {
        return Err(FaucetError::BadRequest("Invalid asset amount.".to_string()));
    }

    // TODO: We lock the client for the whole request which leads to blocking of other requests.
    //       We should find a way to avoid this. The simplest solution would be to create new client
    //       for each request. If this takes too long, we should consider using a pool of clients.
    let mut client = state.client.lock().await;

    // Receive and hex user account id
    let target_account_id = AccountId::from_hex(req.account_id.as_str())
        .map_err(|err| FaucetError::BadRequest(err.to_string()))?;

    // Execute transaction
    info!("Executing mint transaction for account.");
    let (executed_tx, created_note) = client.execute_mint_transaction(
        target_account_id,
        req.is_private_note,
        req.asset_amount,
    )?;

    // Run transaction prover & send transaction to node
    info!("Proving and submitting transaction.");
    let block_height = client.prove_and_submit_transaction(executed_tx).await?;

    let note_id: NoteId = created_note.id();
    let note_details =
        NoteDetails::new(created_note.assets().clone(), created_note.recipient().clone());

    let note_tag = NoteTag::from_account_id(target_account_id, NoteExecutionMode::Local)
        .expect("failed to build note tag for local execution");

    // Serialize note into bytes
    let bytes = NoteFile::NoteDetails {
        details: note_details,
        after_block_num: block_height,
        tag: Some(note_tag),
    }
    .to_bytes();

    info!("A new note has been created: {}", note_id);

    // Send generated note to user
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=note.mno")
        .header("Note-Id", note_id.to_string())
        .body(body::boxed(Full::from(bytes)))
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))
}

pub async fn get_index(state: State<FaucetState>) -> Result<impl IntoResponse, FaucetError> {
    get_static_file(state, Path("index.html".to_string())).await
}

pub async fn get_static_file(
    State(state): State<FaucetState>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, FaucetError> {
    info!(target: COMPONENT, path, "Serving static file");

    let static_file = state.static_files.get(path.as_str()).ok_or(FaucetError::NotFound(path))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, static_file.mime_type)
        .body(body::boxed(Full::from(static_file.data)))
        .map_err(|err| FaucetError::InternalServerError(err.to_string()))
}
