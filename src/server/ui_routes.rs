use super::errors::ServerError;
use crate::server::ui_types::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::SqlitePool;
use tracing::instrument;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = "/api/runs",
    params(
        ("startDate" = Option<String>, Query, description = "Start date (String of NaiveDateTime)"),
        ("endDate" = Option<String>, Query, description = "End date (String of NaiveDateTime)"),
    ),
    responses(
        (status = 200, description = "Data for runs within date range.", body = RunsResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of runs")]
pub async fn get_runs(
    State(pool): State<SqlitePool>,
    Query(params): Query<RunParams>,
) -> anyhow::Result<Json<RunsResponse>, ServerError> {
    // Returns runs with *not* scenario_name
    Ok(Json(RunsResponse {
        data: vec![
            Runs {
                metrics: vec![
                    Metric {
                        metric_type: "CO2".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 0.81,
                    },
                    Metric {
                        metric_type: "POWER".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 1.23,
                    },
                    Metric {
                        metric_type: "CPU".to_string(),
                        type_field: MetricType::AVERAGE,
                        value: 2.34,
                    },
                ],
                start_time: "2023-06-15T10:30:00.000Z".to_string(),
                id: "run_123".to_string(),
                end_time: "2023-06-15T11:00:00.000Z".to_string(),
            },
            Runs {
                metrics: vec![
                    Metric {
                        metric_type: "CO2".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 0.92,
                    },
                    Metric {
                        metric_type: "POWER".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 1.45,
                    },
                    Metric {
                        metric_type: "CPU".to_string(),
                        type_field: MetricType::AVERAGE,
                        value: 2.67,
                    },
                ],
                start_time: "2023-06-16T09:15:00.000Z".to_string(),
                id: "run_456".to_string(),
                end_time: "2023-06-16T09:45:00.000Z".to_string(),
            },
        ],
    }))
}

#[utoipa::path(
    get,
    path = "/api/runs/{runId}",
    params(
        ("runId",description = "ID of the run")
    ),
    responses(
        (status = 200, description = "List of scenarios for the specified run", body = RunWithScenarioResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of scenarios for specific run")]
pub async fn get_scenarios_for_run(
    Path(run_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<RunWithScenarioResponse>, ServerError> {
    // Returns runs *with* scenario_name
    Ok(Json(RunWithScenarioResponse {
        data: vec![
            RunWithScenario {
                metrics: vec![
                    Metric {
                        metric_type: "CO2".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 0.81,
                    },
                    Metric {
                        metric_type: "POWER".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 1.23,
                    },
                    Metric {
                        metric_type: "CPU".to_string(),
                        type_field: MetricType::AVERAGE,
                        value: 2.34,
                    },
                ],
                start_time: "2023-06-15T10:30:00.000Z".to_string(),
                id: "run_123".to_string(),
                end_time: "2023-06-15T11:00:00.000Z".to_string(),
                scenario_name: Some("Name1".to_string()),
            },
            RunWithScenario {
                metrics: vec![
                    Metric {
                        metric_type: "CO2".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 0.92,
                    },
                    Metric {
                        metric_type: "POWER".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 1.45,
                    },
                    Metric {
                        metric_type: "CPU".to_string(),
                        type_field: MetricType::TOTAL,
                        value: 2.67,
                    },
                ],
                scenario_name: Some("Name2".to_string()),
                start_time: "2023-06-16T09:15:00.000Z".to_string(),
                id: "run_456".to_string(),
                end_time: "2023-06-16T09:45:00.000Z".to_string(),
            },
        ],
    }))
}

#[utoipa::path(
    get,
    path = "/api/scenarios/{scenarioId}",
    params(
        ("scenarioId", description = "ID of the scenario")
    ),
    responses(
        (status = 200, description = "List of iterations for the specified scenario", body = ScenarioResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get list of iterations for specific scenario")]
pub async fn get_iterations(
    Path(scenario_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> anyhow::Result<Json<ScenarioResponse>, ServerError> {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/metrics",
    params(
        ("startDate" = Option<String>, Query, description = "Start date (String of NaiveDateTime)"),
        ("endDate" = Option<String>, Query, description = "End date (String of NaiveDateTime)"),
        ("runId" = Option<String>, Query, description = "Run ID"),
        ("scenarioId" = Option<String>, Query, description = "Scenario ID "),
        ("type" = MetricType, Query, description = "Type of metric")
    ),
    responses(
        (status = 200, description = "Metrics based on the specified type (total or average) for runs, a specific run, or a specific scenario.", body = MetricResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get metrics for runs, a specific run or scenario")]
pub async fn get_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<Json<MetricResponse>, ServerError> {
    todo!()
}

#[utoipa::path(
    get,
    path = "/api/cpu-metrics",
    params(
        ("startDate" = Option<String>, Query, description = "Start date (String of NaiveDateTime)"),
        ("endDate" = Option<String>, Query, description = "End date (String of NaiveDateTime)"),
        ("runId" = Option<String>, Query, description = "Run ID"),
        ("scenarioId" = Option<String>, Query, description = "Scenario ID")
    ),
    responses(
        (status = 200, description = "CPU metrics for runs, a specific run, or scenario", body = CpuMetricsResponse),
        (status = 500, description = "Internal Server Error")
    )
)]
#[instrument(name = "Get cpu-metrics for runs, a specific run, or scenario")]
pub async fn get_cpu_metrics(
    State(pool): State<SqlitePool>,
    Json(payload): Json<String>,
) -> anyhow::Result<Json<CpuMetricsResponse>, ServerError> {
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
    components(schemas(
        RunParams,
        RunsResponse,
        ScenarioResponse,
        MetricType,
        Scenario,
        GetMetricsParams,
        GetCpuMetricsParams,
        RunWithScenarioResponse,
        CpuMetricsResponse,
        MetricResponse
    ))
)]
pub struct ApiDoc;
