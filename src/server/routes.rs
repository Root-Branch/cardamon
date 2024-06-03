use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use cardamon::data_access::cpu_metrics::CpuMetrics;
use sqlx::SqlitePool;
use tracing::instrument;

use super::errors::ServerError;

// Must receive data from src/data_access/cpu_metrics.rs in this format:
/*
   async fn fetch(&self, id: &str) -> anyhow::Result<Option<CpuMetrics>> {
        self.client
            .get(format!("{}/cpu_metrics/{id}", self.base_url))
            .send()
            .await?
            .json::<Option<CpuMetrics>>()
            .await
            .context("Error fetching cpu metrics with id {id} from remote server")
    }

    async fn persist(&self, metrics: &CpuMetrics) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/cpu_metrics", self.base_url))
            .json(metrics)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting cpu metrics to remote server")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.client
            .delete(format!("{}/cpu_metrics/{id}", self.base_url))
            .send()
            .await
            .map(|_| ())
            .context("Error deleting cpu metrics with id {id}")
    }
*/
#[instrument(name = "Fetch metrics by ID")]
pub async fn fetch_metrics(
    Path(metrics_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, ServerError> {
    // Impl response because we don't want to define
    // server stuff ( impl response for Cpumetrics ) in
    // our data_access file
    let metrics = fetch_metrics_from_db(&pool, &metrics_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch metrics: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Fetched metrics: {:?}", metrics);
    Ok(Json(metrics))
}

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

#[instrument(name = "Delete metrics by ID")]
pub async fn delete_metrics(
    Path(metrics_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Deleting metrics with ID: {}", metrics_id);
    delete_metrics_from_db(&pool, &metrics_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete metrics: {:?}", e);
            ServerError::DatabaseError(e)
        })?;
    tracing::info!("Metrics deleted successfully");
    Ok("Metrics deleted".to_string())
}
async fn fetch_metrics_from_db(
    pool: &SqlitePool,
    metrics_id: &str,
) -> Result<CpuMetrics, sqlx::Error> {
    let result = sqlx::query_as!(
        CpuMetrics,
        "SELECT * FROM cpu_metrics WHERE id = ?",
        metrics_id
    )
    .fetch_one(pool)
    .await?;
    Ok(result)
}
async fn insert_metrics_into_db(
    pool: &SqlitePool,
    metrics: &CpuMetrics,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO cpu_metrics (id, cardamon_run_id, process_id, process_name, cpu_usage, total_usage, core_count, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        metrics.id,
        metrics.cardamon_run_id,
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

async fn delete_metrics_from_db(pool: &SqlitePool, metrics_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM cpu_metrics WHERE id = ?", metrics_id)
        .execute(pool)
        .await?;
    Ok(())
}
