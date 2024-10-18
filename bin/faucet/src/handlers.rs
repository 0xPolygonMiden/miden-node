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
use tracing::{error, info};

use crate::{
    errors::{ErrorHelper, HandlerError},
    state::FaucetState,
    COMPONENT,
};

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
) -> Result<impl IntoResponse, HandlerError> {
    info!(
        target: COMPONENT,
        account_id = %req.account_id,
        is_private_note = %req.is_private_note,
        asset_amount = %req.asset_amount,
        "Received a request",
    );

    // Check that the amount is in the asset amount options
    if !state.config.asset_amount_options.contains(&req.asset_amount) {
        return Err(HandlerError::BadRequest("Invalid asset amount".to_string()));
    }

    let mut client = state.client.lock().await;

    // Receive and hex user account id
    let target_account_id = AccountId::from_hex(req.account_id.as_str())
        .map_err(|err| HandlerError::BadRequest(err.to_string()))?;

    let (created_note, block_height) = loop {
        let mut faucet_account = client.data_store().faucet_account();

        // Execute transaction
        info!(target: COMPONENT, "Executing mint transaction for account.");
        let (executed_tx, created_note) = client.execute_mint_transaction(
            target_account_id,
            req.is_private_note,
            req.asset_amount,
        )?;

        let prev_hash = faucet_account.hash();

        faucet_account
            .apply_delta(executed_tx.account_delta())
            .or_fail("Failed to apply faucet account delta")?;

        let new_hash = faucet_account.hash();

        // Run transaction prover & send transaction to node
        info!(target: COMPONENT, "Proving and submitting transaction.");
        match client.prove_and_submit_transaction(executed_tx.clone()).await {
            Ok(block_height) => {
                break (created_note, block_height);
            },
            Err(err) => {
                error!(
                    target: COMPONENT,
                    %err,
                    "Failed to prove and submit transaction",
                );

                info!(target: COMPONENT, "Trying to request account state from the node...");
                let (got_faucet_account, block_num) = client.request_account_state().await?;
                let got_new_hash = got_faucet_account.hash();
                info!(
                    target: COMPONENT,
                    %prev_hash,
                    %new_hash,
                    %got_new_hash,
                    "Received new account state from the node",
                );
                // If the hash hasn't changed, then the account's state we had is correct,
                // and we should not try to execute the transaction again. We can just return error
                // to the caller.
                if new_hash == prev_hash {
                    return Err(err.into());
                }
                // If the new hash from the node is the same, as we expected to have, then
                // transaction was successfully executed despite the error. Don't need to retry.
                if new_hash == got_new_hash {
                    break (created_note, block_num);
                }

                client.data_store().update_faucet_state(got_faucet_account).await?;
            },
        }
    };

    let note_id: NoteId = created_note.id();
    let note_details =
        NoteDetails::new(created_note.assets().clone(), created_note.recipient().clone());

    let note_tag = NoteTag::from_account_id(target_account_id, NoteExecutionMode::Local)
        .or_fail("failed to build note tag for local execution")?;

    // Serialize note into bytes
    let bytes = NoteFile::NoteDetails {
        details: note_details,
        after_block_num: block_height,
        tag: Some(note_tag),
    }
    .to_bytes();

    info!(target: COMPONENT, %note_id, "A new note has been created");

    // Send generated note to user
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=note.mno")
        .header("Note-Id", note_id.to_string())
        .body(body::boxed(Full::from(bytes)))
        .or_fail("Failed to build response")
}

pub async fn get_index(state: State<FaucetState>) -> Result<impl IntoResponse, HandlerError> {
    get_static_file(state, Path("index.html".to_string())).await
}

pub async fn get_static_file(
    State(state): State<FaucetState>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, HandlerError> {
    info!(target: COMPONENT, path, "Serving static file");

    let static_file = state.static_files.get(path.as_str()).ok_or(HandlerError::NotFound(path))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, static_file.mime_type)
        .body(body::boxed(Full::from(static_file.data)))
        .or_fail("Failed to build response")
}
