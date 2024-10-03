use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FaucetError {
    #[error("Failed to start faucet: {0}")]
    StartError(String),

    #[error("Client has submitted a bad request: {0}")]
    BadRequest(String),

    #[error("Failed to configure faucet: {0}")]
    ConfigurationError(String),

    #[error("Server has encountered an internal error: {0}")]
    InternalServerError(String),

    #[error("Failed to create Miden account: {0}")]
    AccountCreationError(String),

    #[error("Page not found: {0}")]
    NotFound(String),
}

impl FaucetError {
    fn status_code(&self) -> StatusCode {
        match *self {
            FaucetError::BadRequest(_) => StatusCode::BAD_REQUEST,
            FaucetError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for FaucetError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let message = match self {
            FaucetError::StartError(msg) => msg,
            FaucetError::BadRequest(msg) => msg,
            FaucetError::ConfigurationError(msg) => msg,
            FaucetError::InternalServerError(msg) => msg,
            FaucetError::AccountCreationError(msg) => msg,
            FaucetError::NotFound(msg) => msg,
        };

        (status_code, [(header::CONTENT_TYPE, mime::TEXT_HTML_UTF_8.as_ref())], message)
            .into_response()
    }
}
