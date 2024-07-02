use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum ServerError {
    DatabaseError(sqlx::Error),
    TimeFormatError(chrono::ParseError),
    #[allow(dead_code)]
    OtherError,
}

impl ServerError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServerError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::OtherError => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::TimeFormatError(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn error_message(&self) -> String {
        match self {
            //ServerError::DatabaseError(e) => format!("Database error: {}", e),
            ServerError::DatabaseError(e) => match e {
                sqlx::Error::RowNotFound => format!("Row not found: {}", e),
                _ => format!("Database error: {}", e),
            },
            ServerError::OtherError => "Un-used error".to_string(),
            ServerError::TimeFormatError(e) => format!("Time parsing error: {}", e),
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
