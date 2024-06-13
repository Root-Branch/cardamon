mod errors;

use axum::{extract::State, Json};
use cardamon::data_access::cpu_metrics::CpuMetrics;
use errors::ServerError;
use sqlx::SqlitePool;
use tracing::instrument;

#[instrument(name = "Persist metrics into database")]
pub async fn persist_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CpuMetrics>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);
    insert_metrics_into_db(&pool, &payload).await.map_err(|e| {
        tracing::error!("Failed to persist metrics: {:?}", e);
        ServerError::DatabaseError(e)
    })?;
    tracing::info!("Metrics persisted successfully");
    Ok("Metrics persisted".to_string())
}

async fn insert_metrics_into_db(
    pool: &SqlitePool,
    metrics: &CpuMetrics,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO cpu_metrics (run_id, process_id, process_name, cpu_usage, total_usage, core_count, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?)",
        metrics.run_id,
        metrics.process_id,
        metrics.process_name,
        metrics.cpu_usage,
        metrics.total_usage,
        metrics.core_count,
        metrics.timestamp
    )
    .execute(pool)
    .await?;
    Ok(())
}
