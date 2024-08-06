use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum ServerError {
    DatabaseError(sqlx::Error),
    InternalServerError(String),
    #[allow(dead_code)]
    AnyhowError(anyhow::Error),
    NotFound(String),
}

impl ServerError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServerError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::AnyhowError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::NotFound(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn error_message(&self) -> String {
        match self {
            //ServerError::DatabaseError(e) => format!("Database error: {}", e),
            ServerError::DatabaseError(e) => match e {
                sqlx::Error::RowNotFound => format!("Row not found: {}", e),
                _ => format!("Database error: {}", e),
            },
            ServerError::AnyhowError(e) => format!("Anyhow error: {}", e),
            ServerError::NotFound(e) => format!("{} not found", e),
            ServerError::InternalServerError(e) => format!("{} ", e),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error_message())
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status_code(),
            Json(json!({"error": self.error_message()})),
        )
            .into_response()
    }
}
impl From<anyhow::Error> for ServerError {
    fn from(error: anyhow::Error) -> Self {
        ServerError::AnyhowError(error)
    }
}
