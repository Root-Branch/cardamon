use super::errors::ServerError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::NaiveDateTime;
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::instrument;
use utoipa::ToSchema;
use utoipa::{IntoParams, OpenApi};

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct RunParams {
    #[serde(rename = "startDate")]
    start_date: Option<NaiveDateTime>,
    #[serde(rename = "endDate")]
    end_date: Option<NaiveDateTime>,
}

#[utoipa::path(
    get,
    path = "/api/runs",
    params(RunParams),
    responses(
        (status = 200, description = "List of runs"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of runs")]
pub async fn get_runs(
    State(pool): State<SqlitePool>,
    Query(params): Query<RunParams>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/runs/{runId}",
    params(
        ("run_id", description = "ID of the run")
    ),
    responses(
        (status = 200, description = "List of scenarios for the specified run"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of scenarios for specific run")]
pub async fn get_scenarios_for_run(
    Path(run_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/scenarios/{scenarioId}",
    params(
        ("scenario_id", description = "ID of the scenario")
    ),
    responses(
        (status = 200, description = "List of iterations for the specified scenario"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of iterations for specific scenario")]
pub async fn get_iterations(
    Path(scenario_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[derive(Debug, Deserialize, ToSchema)]
enum MetricsType {
    TOTAL,
    AVERAGE,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetMetricsParams {
    #[serde(rename = "startDate")]
    start_date: Option<NaiveDateTime>,
    #[serde(rename = "endDate")]
    end_date: Option<NaiveDateTime>,
    run_id: Option<String>,
    r#type: MetricsType,
}

#[utoipa::path(
    get,
    path = "/api/metrics",
    request_body = String,
    responses(
        (status = 200, description = "Metrics for runs, a specific run, or scenario"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get metrics for runs, a specific run or scenario")]
pub async fn get_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/cpu-metrics",
    request_body = String,
    responses(
        (status = 200, description = "CPU metrics for runs, a specific run, or scenario"),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get cpu-metrics for runs, a specific run, or scenario")]
pub async fn get_cpu_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<String, ServerError> {
    todo!()
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_runs,
        get_scenarios_for_run,
        get_iterations,
        get_metrics,
        get_cpu_metrics
    ),
    components(
        schemas(RunParams, MetricsType, GetMetricsParams)
    ),
    tags(
        (name = "Runs", description = "API endpoints for managing runs"),
        (name = "Scenarios", description = "API endpoints for managing scenarios"),
        (name = "Metrics", description = "API endpoints for retrieving metrics")
    )
)]
pub struct ApiDoc;
