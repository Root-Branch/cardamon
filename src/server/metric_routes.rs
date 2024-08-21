use chrono::Utc;

use super::errors::ServerError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{metrics::Metrics, DAOService, LocalDAOService};
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct WithinParams {
    begin: Option<i64>,
    end: Option<i64>,
}

#[instrument(name = "Fetch CPU metrics within a time range")]
pub async fn fetch_within(
    State(dao_service): State<LocalDAOService>,
    Path(run_id): Path<String>,
    Query(params): Query<WithinParams>,
) -> Result<Json<Vec<Metrics>>, ServerError> {
    let from = params.begin.unwrap_or(0);
    let to = params.end.unwrap_or_else(|| Utc::now().timestamp_millis());

    tracing::debug!(
        "Received request to fetch CPU metrics for run ID: {}, begin: {}, end: {}",
        run_id,
        from,
        to
    );
    let metrics = dao_service
        .metrics()
        .fetch_within(&run_id, from, to)
        .await?;

    tracing::info!("Successfully fetched {} CPU metrics", metrics.len());
    Ok(Json(metrics))
}

#[instrument(name = "Persist metrics into database")]
pub async fn persist_metrics(
    State(dao_service): State<LocalDAOService>,
    Json(payload): Json<Metrics>,
) -> Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);
    dao_service.metrics().persist(&payload).await?;

    tracing::info!("Metrics persisted successfully");
    Ok("Metrics persisted".to_string())
}
