use std::fmt::Debug;

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    RequestError(#[from] tonic::Status),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("client error")]
    ClientError(#[from] ClientError),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),

    #[error("bad request: {0}")]
    BadRequest(String),
}

impl HandlerError {
    fn status_code(&self) -> StatusCode {
        match *self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::ClientError(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(msg) => msg,
            Self::ClientError(_) | Self::Internal(_) => "Internal error",
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
