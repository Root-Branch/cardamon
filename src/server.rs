mod errors;
use chrono::Utc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{cpu_metrics::CpuMetrics, scenario_iteration::ScenarioIteration};
use errors::ServerError;
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::instrument;

// Must receive data from src/data_access/cpu_metrics.rs in this format:
/*

     async fn fetch_within(
        &self,
        cardamon_run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<CpuMetrics>> {
        self.client
            .get(format!(
                "{}/cpu_metrics/{cardamon_run_id}?begin={begin}&end={end}",
                self.base_url
            ))
            .send()
            .await?
            .json::<Vec<CpuMetrics>>()
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

//Start cpu_metric routes
#[derive(Debug, Deserialize)]
pub struct WithinParams {
    begin: Option<i64>,
    end: Option<i64>,
}
#[instrument(name = "Fetch CPU metrics within a time range")]
pub async fn fetch_within(
    Path(cardamon_run_id): Path<String>,
    Query(params): Query<WithinParams>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<Vec<CpuMetrics>>, ServerError> {
    let begin = params.begin.unwrap_or(0);
    let end = params.end.unwrap_or_else(|| Utc::now().timestamp());

    tracing::debug!(
        "Received request to fetch CPU metrics for run ID: {}, begin: {}, end: {}",
        cardamon_run_id,
        begin,
        end
    );

    let metrics = fetch_metrics_within_range(&pool, &cardamon_run_id, begin, end)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch metrics from database: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Successfully fetched {} CPU metrics", metrics.len());
    Ok(Json(metrics))
}

async fn fetch_metrics_within_range(
    pool: &SqlitePool,
    cardamon_run_id: &str,
    begin: i64,
    end: i64,
) -> Result<Vec<CpuMetrics>, sqlx::Error> {
    let metrics = sqlx::query_as!(
        CpuMetrics,
        "SELECT * FROM cpu_metrics WHERE run_id = ? AND timestamp BETWEEN ? AND ?",
        cardamon_run_id,
        begin,
        end
    )
    .fetch_all(pool)
    .await?;
    Ok(metrics)
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

async fn delete_metrics_from_db(pool: &SqlitePool, metrics_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM cpu_metrics WHERE run_id = ?", metrics_id)
        .execute(pool)
        .await?;
    Ok(())
}

// Below routes must confirm to these routes found in src/data_access/scenario_run.rs
/*
   async fn fetch_last(&self, _name: &str, _n: u32) -> anyhow::Result<Vec<ScenarioRun>> {
        todo!()
    }

    async fn fetch(&self, id: &str) -> anyhow::Result<Option<ScenarioRun>> {
        self.client
            .get(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .json::<Option<ScenarioRun>>()
            .await
            .context("Error fetching scenario with id {id} from remote server")
    }

    async fn persist(&self, scenario: &ScenarioRun) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/scenario", self.base_url))
            .json(scenario)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting scenario to remote server")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.client
            .delete(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error deleting scenario from remote server")
    }
*/
#[instrument(name = "Fetch last scenario_run")]
pub async fn scenario_run_fetch_last(
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<ScenarioIteration>, ServerError> {
    tracing::debug!("Received request to fetch last scenario run");

    let scenario_run = fetch_last_scenario_run(&pool).await.map_err(|e| {
        tracing::error!("Failed to fetch last scenario run from database: {:?}", e);
        ServerError::DatabaseError(e)
    })?;

    tracing::info!("Successfully fetched last scenario run");
    Ok(Json(scenario_run))
}

#[instrument(name = "Fetch scenario run by id")]
pub async fn scenario_run_fetch_by_id(
    Path(scenario_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<ScenarioIteration>, ServerError> {
    tracing::debug!(
        "Received request to fetch scenario run with ID: {}",
        scenario_id
    );

    let scenario_run = fetch_scenario_run_by_id(&pool, &scenario_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch scenario run from database: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Successfully fetched scenario run");
    Ok(Json(scenario_run))
}

#[instrument(name = "Persist scenario_run")]
pub async fn scenario_iteration_persist(
    State(pool): State<SqlitePool>,
    Json(payload): Json<ScenarioIteration>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);

    insert_scenario_run_into_db(&pool, &payload)
        .await
        .map_err(|e| {
            tracing::error!("Failed to persist scenario run: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Scenario run persisted successfully");
    Ok("Scenario run persisted".to_string())
}

#[instrument(name = "Scenario run delete by id")]
pub async fn scenario_run_delete_by_id(
    Path(scenario_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Deleting scenario run with ID: {}", scenario_id);

    delete_scenario_run_from_db(&pool, &scenario_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete scenario run: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Scenario run deleted successfully");
    Ok("Scenario run deleted".to_string())
}

async fn fetch_last_scenario_run(pool: &SqlitePool) -> Result<ScenarioIteration, sqlx::Error> {
    let scenario_run = sqlx::query_as!(
        ScenarioIteration,
        "SELECT * FROM scenario_iteration ORDER BY start_time DESC LIMIT 1"
    )
    .fetch_one(pool)
    .await?;
    Ok(scenario_run)
}

async fn fetch_scenario_run_by_id(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<ScenarioIteration, sqlx::Error> {
    let scenario_run = sqlx::query_as!(
        ScenarioIteration,
        "SELECT * FROM scenario_iteration WHERE run_id = ?",
        scenario_id
    )
    .fetch_one(pool)
    .await?;
    Ok(scenario_run)
}

async fn insert_scenario_run_into_db(
    pool: &SqlitePool,
    scenario_run: &ScenarioIteration,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO scenario_iteration (run_id, scenario_name, iteration, start_time, stop_time) VALUES (?, ?, ?, ?, ?)",
        scenario_run.run_id,
        scenario_run.scenario_name,
        scenario_run.iteration,
        scenario_run.start_time,
        scenario_run.stop_time
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn delete_scenario_run_from_db(
    pool: &SqlitePool,
    scenario_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM scenario_iteration WHERE run_id = ?",
        scenario_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
