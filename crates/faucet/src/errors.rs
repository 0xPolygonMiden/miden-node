use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use derive_more::Display;

#[derive(Debug, Display)]
pub enum FaucetError {
    BadRequest(String),
    InternalServerError(String),
    InitializationError(String),
    DatabaseError(String),
    SyncError(String),
    ClientCreationError(String),
    AccountCreationError(String),
}

impl error::ResponseError for FaucetError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let message = match self {
            FaucetError::BadRequest(msg) => msg,
            FaucetError::InternalServerError(msg) => msg,
            FaucetError::SyncError(msg) => msg,
            FaucetError::InitializationError(msg) => msg,
            FaucetError::ClientCreationError(msg) => msg,
            FaucetError::AccountCreationError(msg) => msg,
            FaucetError::DatabaseError(msg) => msg,
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
