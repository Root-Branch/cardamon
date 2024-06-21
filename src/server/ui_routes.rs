use super::errors::ServerError;
use crate::server::ui_types::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{cpu_metrics::CpuMetrics, scenario_iteration::ScenarioIteration};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::SqlitePool;
use tracing::{debug, info, instrument};
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
    let end_timestamp = match params.end_date {
        Some(date_str) => NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S%.3fZ")
            .map_err(|e| {
                tracing::error!("Failed to parse incoming time {:?}", e);
                ServerError::TimeFormatError(e)
            })?
            .and_utc()
            .timestamp_millis(),
        None => Utc::now().timestamp_millis(),
    };

    let start_timestamp = match params.start_date {
        Some(date_str) => NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S%.3fZ")
            .map_err(|e| {
                tracing::error!("Failed to parse incoming time {:?}", e);
                ServerError::TimeFormatError(e)
            })?
            .and_utc()
            .timestamp_millis(),
        None => 0,
    };
    debug!(
        "End timestamp {}, Start timestamp {}",
        end_timestamp, start_timestamp
    );
    // Get each iteration
    let scenario_iterations =
        fetch_scenario_iteration_within_range(&pool, start_timestamp, end_timestamp)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch runs from database {:?}", e);
                ServerError::DatabaseError(e)
            })?;
    info!("Scenario_iterations {:?}", scenario_iterations);
    // Fetch all CPU metrics for these run_ids in a single query
    let all_cpu_metrics = fetch_metrics_for_multiple_runs(&pool, start_timestamp, end_timestamp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch CPU metrics from database {:?}", e);
            ServerError::DatabaseError(e)
        })?;
    let mapped_iterations = create_scenario_metrics_vec(scenario_iterations, all_cpu_metrics);
    let mut data: Vec<Runs> = Vec::new();

    for (iteration, metrics) in mapped_iterations {
        let mut run_metrics = Vec::new();

        // Calculate total CPU usage
        let total_cpu = metrics.iter().map(|m| m.cpu_usage).sum::<f64>();
        run_metrics.push(Metric {
            metric_type: "CPU".to_string(),
            type_field: MetricType::TOTAL,
            value: total_cpu,
        });
        run_metrics.push(Metric {
            metric_type: "CPU".to_string(),
            type_field: MetricType::AVERAGE,
            value: total_cpu / metrics.len() as f64,
        });

        // Placeholder as we don't have other metrics now
        run_metrics.push(Metric {
            metric_type: "CO2".to_string(),
            type_field: MetricType::TOTAL,
            value: 0.81, // placeholder value
        });
        run_metrics.push(Metric {
            metric_type: "POWER".to_string(),
            type_field: MetricType::AVERAGE,
            value: 1.23, // placeholder value
        });

        // Convert timestamps to ISO 8601 format
        let start_time = DateTime::from_timestamp_millis(iteration.start_time)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let end_time = DateTime::from_timestamp_millis(iteration.stop_time)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        let run = Runs {
            metrics: run_metrics,
            start_time,
            id: iteration.run_id,
            end_time,
        };

        data.push(run);
    }

    Ok(Json(RunsResponse { data }))
}
fn create_scenario_metrics_vec(
    scenario_iterations: Vec<ScenarioIteration>,
    all_cpu_metrics: Vec<CpuMetrics>,
) -> Vec<(ScenarioIteration, Vec<CpuMetrics>)> {
    scenario_iterations
        .into_iter()
        .map(|scenario| {
            let matching_metrics = all_cpu_metrics
                .iter()
                .filter(|metric| {
                    metric.timestamp >= scenario.start_time
                        && metric.timestamp <= scenario.stop_time
                })
                .cloned()
                .collect();
            (scenario, matching_metrics)
        })
        .collect()
}
async fn fetch_metrics_for_multiple_runs(
    pool: &SqlitePool,
    begin: i64,
    end: i64,
) -> Result<Vec<CpuMetrics>, sqlx::Error> {
    let metrics = sqlx::query_as!(
        CpuMetrics,
        "SELECT * FROM cpu_metrics WHERE timestamp BETWEEN ? AND ?",
        begin,
        end
    )
    .fetch_all(pool)
    .await?;
    Ok(metrics)
}
async fn fetch_scenario_iteration_within_range(
    pool: &SqlitePool,
    begin: i64,
    end: i64,
) -> Result<Vec<ScenarioIteration>, sqlx::Error> {
    let runs = sqlx::query_as!(
        ScenarioIteration,
        "SELECT * FROM scenario_iteration WHERE start_time >= ? AND stop_time <= ?",
        begin,
        end
    )
    .fetch_all(pool)
    .await?;
    Ok(runs)
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
        Metric,
        RunWithScenario,
        Runs,
        RunsResponse,
        ScenarioResponse,
        MetricType,
        CpuMetric,
        Scenario,
        GetMetricsParams,
        GetCpuMetricsParams,
        RunWithScenarioResponse,
        CpuMetricsResponse,
        MetricResponse
    ))
)]
pub struct ApiDoc;
