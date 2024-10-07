use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("Failed to start faucet: {0}")]
    FaucetFailedToStart(String),

    #[error("Failed to initialize client: {0}")]
    ClientInitFailed(String),

    #[error("Failed to configure faucet: {0}")]
    ConfigurationError(String),

    #[error("Failed to create Miden account: {0}")]
    AccountCreationError(String),
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Client has submitted a bad request: {0}")]
    BadRequest(String),

    #[error("Server has encountered an internal error: {0}")]
    InternalServerError(String),

    #[error("Page not found: {0}")]
    NotFound(String),
}

impl ProcessError {
    fn status_code(&self) -> StatusCode {
        match *self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(msg) => msg,
            Self::InternalServerError(msg) => msg,
            Self::NotFound(msg) => msg,
        }
        .to_string()
    }
}

impl IntoResponse for ProcessError {
    fn into_response(self) -> Response {
        (
            self.status_code(),
            [(header::CONTENT_TYPE, mime::TEXT_HTML_UTF_8.as_ref())],
            self.message(),
        )
            .into_response()
    }
}
