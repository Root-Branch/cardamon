use axum::{http::StatusCode, response::IntoResponse};
use std::fmt;

#[derive(Debug)]
// TODO: Split server error into different types
pub struct ServerError(pub anyhow::Error);

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong! \n{}", self.0),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(error: anyhow::Error) -> Self {
        ServerError(error)
    }
}
