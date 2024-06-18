use super::errors::ServerError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
// Format of file:
// Params / inputs struct
// handler function
// database - call ( if needed )
// Return struct
use chrono::NaiveDateTime;
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::instrument;
// get_runs param input
#[derive(Debug, Deserialize)]
pub struct RunParams {
    #[serde(rename = "startDate")]
    start_date: Option<NaiveDateTime>,
    #[serde(rename = "endDate")]
    end_date: Option<NaiveDateTime>,
}
#[instrument(name = "Get list of runs")]
pub async fn get_runs(
    State(pool): State<SqlitePool>,
    Query(params): Query<RunParams>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[instrument(name = "Get list of scenarios for specific run")]
pub async fn get_scenarios_for_run(
    Path(run_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}
#[instrument(name = "Get list of iterations for specific scenario")]
pub async fn get_iterations(
    Path(scenario_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}
#[derive(Debug, Deserialize)]
enum MetricsType {
    TOTAL,
    AVERAGE,
}
#[derive(Debug, Deserialize)]
pub struct GetMetricsParams {
    #[serde(rename = "startDate")]
    start_date: Option<NaiveDateTime>,
    #[serde(rename = "endDate")]
    end_date: Option<NaiveDateTime>,
    run_id: Option<String>,
    r#type: MetricsType, // Escape type as it's a keyword
}
#[instrument(name = "Get metrics for runs, a specific run or scenario")]
pub async fn get_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[instrument(name = "Get cpu-metrics for runs, a specific run, or scenario")]
pub async fn get_cpu_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}
