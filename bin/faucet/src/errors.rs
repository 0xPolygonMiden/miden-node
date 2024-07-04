use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
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
}

impl error::ResponseError for FaucetError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let message = match self {
            FaucetError::StartError(msg) => msg.to_string(),
            FaucetError::BadRequest(msg) => msg.to_string(),
            FaucetError::ConfigurationError(msg) => msg.to_string(),
            FaucetError::InternalServerError(msg) => msg.to_string(),
            FaucetError::AccountCreationError(msg) => msg.to_string(),
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
