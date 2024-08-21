use super::errors::ServerError;
use axum::{extract::State, Json};
use cardamon::data_access::{run::Run, DAOService, LocalDAOService};
use tracing::instrument;

#[instrument(name = "Persist run into database")]
pub async fn persist(
    State(dao_service): State<LocalDAOService>,
    Json(payload): Json<Run>,
) -> Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);
    dao_service.runs().persist(&payload).await?;

    tracing::info!("Run persisted successfully");
    Ok("Run persisted".to_string())
}
