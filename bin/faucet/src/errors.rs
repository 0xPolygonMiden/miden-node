use std::fmt::{Debug, Display};

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Wrapper for implementing `Error` trait for errors, which do not implement it, like
/// [miden_objects::crypto::utils::DeserializationError] and other error types from `miden-base`.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ImplError<E: Display + Debug>(pub E);

#[derive(Debug, Error)]
#[error("Client error: {0:#}")]
pub struct ClientError(#[from] anyhow::Error);

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Node client error: {0}")]
    ClientError(#[from] ClientError),

    #[error("Client has submitted a bad request: {0}")]
    BadRequest(String),

    #[error("Server has encountered an internal error: {0}")]
    InternalServerError(String),

    #[error("Page not found: {0}")]
    NotFound(String),
}

impl HandlerError {
    fn status_code(&self) -> StatusCode {
        match *self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::ClientError(_) | Self::InternalServerError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            },
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(msg) => msg,
            Self::ClientError(_) | Self::InternalServerError(_) => "Error processing request",
            Self::NotFound(msg) => msg,
        }
        .to_string()
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        (
            self.status_code(),
            [(header::CONTENT_TYPE, mime::TEXT_HTML_UTF_8.as_ref())],
            self.message(),
        )
            .into_response()
    }
}

pub trait ErrorHelper<T, E: std::error::Error> {
    fn or_fail(self, message: impl Display) -> Result<T, E>;
}

impl<T, E: std::error::Error> ErrorHelper<T, HandlerError> for Result<T, E> {
    fn or_fail(self, message: impl Display) -> Result<T, HandlerError> {
        self.map_err(|err| HandlerError::InternalServerError(format!("{message}: {err}")))
    }
}
