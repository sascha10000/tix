use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Forbidden,
    BadRequest(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not found: {msg}"),
            AppError::Forbidden => write!(f, "Forbidden"),
            AppError::BadRequest(msg) => write!(f, "Bad request: {msg}"),
            AppError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::NotFound(msg) => {
                HttpResponse::NotFound().body(format!("404 Not Found: {msg}"))
            }
            AppError::Forbidden => HttpResponse::Forbidden().body("403 Forbidden"),
            AppError::BadRequest(msg) => {
                HttpResponse::BadRequest().body(format!("400 Bad Request: {msg}"))
            }
            AppError::Internal(msg) => {
                eprintln!("Internal error: {msg}");
                HttpResponse::InternalServerError().body("500 Internal Server Error")
            }
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
