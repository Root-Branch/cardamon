use chrono::Utc;

use super::errors::ServerError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{iteration::Iteration, metrics::Metrics};
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::instrument;

// Must receive data from src/data_access/cpu_metrics.rs in this format:
/*

     async fn fetch_within(
        &self,
        run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<CpuMetrics>> {
        self.client
            .get(format!(
                "{}/cpu_metrics/{run_id}?begin={begin}&end={end}",
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
    Path(run_id): Path<String>,
    Query(params): Query<WithinParams>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<Vec<Metrics>>, ServerError> {
    let begin = params.begin.unwrap_or(0);
    let end = params.end.unwrap_or_else(|| Utc::now().timestamp_millis());

    tracing::debug!(
        "Received request to fetch CPU metrics for run ID: {}, begin: {}, end: {}",
        run_id,
        begin,
        end
    );

    let metrics = fetch_metrics_within_range(&pool, &run_id, begin, end)
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
    run_id: &str,
    begin: i64,
    end: i64,
) -> Result<Vec<Metrics>, sqlx::Error> {
    let metrics = sqlx::query_as!(
        Metrics,
        "SELECT * FROM metrics WHERE run_id = ? AND time_stamp BETWEEN ? AND ?",
        run_id,
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
    Json(payload): Json<Metrics>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);
    insert_metrics_into_db(&pool, &payload).await.map_err(|e| {
        tracing::error!("Failed to persist metrics: {:?}", e);
        ServerError::DatabaseError(e)
    })?;
    tracing::info!("Metrics persisted successfully");
    Ok("Metrics persisted".to_string())
}

async fn insert_metrics_into_db(pool: &SqlitePool, metrics: &Metrics) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO metrics (run_id, process_id, process_name, cpu_usage, cpu_total_usage, cpu_core_count, time_stamp) VALUES (?, ?, ?, ?, ?, ?, ?)",
        metrics.run_id,
        metrics.process_id,
        metrics.process_name,
        metrics.cpu_usage,
        metrics.cpu_total_usage,
        metrics.cpu_core_count,
        metrics.time_stamp
    )
    .execute(pool)
    .await?;
    Ok(())
}

// Below routes must confirm to these routes found in src/data_access/scenario_iteration.rs
/*
   async fn fetch_last(&self, _name: &str, _n: u32) -> anyhow::Result<Vec<ScenarioIteration>> {
        todo!()
    }

    async fn fetch(&self, id: &str) -> anyhow::Result<Option<ScenarioIteration>> {
        self.client
            .get(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .json::<Option<ScenarioIteration>>()
            .await
            .context("Error fetching scenario with id {id} from remote server")
    }

    async fn persist(&self, scenario: &ScenarioIteration) -> anyhow::Result<()> {
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
#[instrument(name = "Fetch last scenario_iteration")]
pub async fn scenario_iteration_fetch_last(
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<Iteration>, ServerError> {
    tracing::debug!("Received request to fetch last scenario run");

    let scenario_iteration = fetch_last_scenario_iteration(&pool).await.map_err(|e| {
        tracing::error!("Failed to fetch last scenario run from database: {:?}", e);
        ServerError::DatabaseError(e)
    })?;

    tracing::info!("Successfully fetched last scenario run");
    Ok(Json(scenario_iteration))
}

#[instrument(name = "Persist scenario iteration")]
pub async fn scenario_iteration_persist(
    State(pool): State<SqlitePool>,
    Json(payload): Json<Iteration>,
) -> anyhow::Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);

    insert_scenario_iteration_into_db(&pool, &payload)
        .await
        .map_err(|e| {
            tracing::error!("Failed to persist scenario run: {:?}", e);
            ServerError::DatabaseError(e)
        })?;

    tracing::info!("Scenario run persisted successfully");
    Ok("Scenario run persisted".to_string())
}

#[inline]
async fn fetch_last_scenario_iteration(pool: &SqlitePool) -> Result<Iteration, sqlx::Error> {
    let scenario_iteration = sqlx::query_as!(
        Iteration,
        "SELECT * FROM iteration ORDER BY start_time DESC LIMIT 1"
    )
    .fetch_one(pool)
    .await?;
    Ok(scenario_iteration)
}

async fn insert_scenario_iteration_into_db(
    pool: &SqlitePool,
    scenario_iteration: &Iteration,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO iteration (run_id, scenario_name, iteration, start_time, stop_time) VALUES (?, ?, ?, ?, ?)",
        scenario_iteration.run_id,
        scenario_iteration.scenario_name,
        scenario_iteration.iteration,
        scenario_iteration.start_time,
        scenario_iteration.stop_time
    )
    .execute(pool)
    .await?;
    Ok(())
}
