use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use miden_client::errors::ClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FaucetError {
    /// Client has submitted a bad request
    #[error("Client has submitted a bad request: {0}")]
    BadRequest(String),

    /// Server has encountered an internal error
    #[error("Server has encountered an internal error: {0}")]
    InternalServerError(String),

    /// Database has encountered an error
    #[error("Database has encountered an error: {0}")]
    DatabaseError(String),

    /// Encountered an error while trying to sync state
    #[error("Failed to sync state: {0}")]
    SyncError(ClientError),

    /// Encountered an error during Miden clien creation
    #[error("Failed to create Miden client: {0}")]
    ClientCreationError(ClientError),

    /// Encountered an error during Miden account creation
    #[error("Failed to create Miden account: {0}")]
    AccountCreationError(String),
}

impl error::ResponseError for FaucetError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let message = match self {
            FaucetError::BadRequest(msg) => msg.to_string(),
            FaucetError::InternalServerError(msg) => msg.to_string(),
            FaucetError::SyncError(msg) => msg.to_string(),
            FaucetError::ClientCreationError(msg) => msg.to_string(),
            FaucetError::AccountCreationError(msg) => msg.to_string(),
            FaucetError::DatabaseError(msg) => msg.to_string(),
        };

        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(message.to_owned())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match *self {
            FaucetError::BadRequest(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
