use crate::{
    dao::{self, iteration::RunId}, dataset::DatasetBuilder, server::{
        errors::ServerError,
        types::{
            Iteration, Pagination, Scenario, Scenario5Average, ScenarioParams, ScenarioResponse, ScenarioRun, ScenariosParams, ScenariosResponse, Usage
        }
    }
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tracing::{instrument, debug, info};

#[instrument(name = "Get list of scenarios")]
pub async fn get_scenarios(
    State(db): State<DatabaseConnection>,
    Query(params): Query<ScenariosParams>,
) -> Result<Json<ScenariosResponse>, ServerError> {
    let begin = params.from_date.unwrap_or(0);
    let end = params
        .to_date
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let page = params.page.unwrap_or(1);
    let page = page - 1; // DB needs -1 indexing
    let limit = params.limit.unwrap_or(5);

    info!("Fetching scenarios between {} and {}", begin, end);

    let dataset = match &params.search_query {
        Some(query) => {
            DatasetBuilder::new(&db)
                .scenarios_by_name(query)
                .all()
                .last_n_runs(5)
                .await?
        }
        None => {
            DatasetBuilder::new(&db)
                .scenarios_in_range(begin, end)
                .all()
                .last_n_runs(5)
                .await?
        }
    };

    let total_scenarios = dataset.total_unique_scenarios() as u32;
    let total_pages = dataset.total_pages(limit); // Returns correct number ( 1 based indexing )
    let paginated_scenarios = dataset.paginated_unique_scenarios(page, limit);

    debug!("Processing {} scenarios", paginated_scenarios.len());

    let mut scenario_responses = Vec::new();
    for scenario_name in paginated_scenarios {
        let last_runs = dataset.last_n_runs_for_scenario(&scenario_name, 5);

        let mut scenario_map: HashMap<i32, Vec<Iteration>> = HashMap::new();
        for run in &last_runs {
            scenario_map
                .entry(run.iteration().run_id)
                .or_default()
                .push(Iteration {
                    run_id: run.iteration().run_id,
                    scenario_name: run.iteration().scenario_name.clone(),
                    iteration: run.iteration().count,
                    start_time: run.iteration().start_time,
                    stop_time: run.iteration().stop_time,
                    usage: None,
                    // Use /scenario/ID for this
                    /*
                    usage: Some(
                        run.metrics()
                            .iter()
                            .map(|m| Usage {
                                cpu_usage: m.cpu_usage,
                                timestamp: m.time_stamp,
                            })
                            .collect(),
                    ),
                    */
                });
        }

        let avg_co2_emission: f64 = 2.0; // Placeholder value
        let avg_power_consumption: f64 = 2.0; // Placeholder value
        let (total_cpu_usage, total_metrics) = last_runs
            .iter()
            .flat_map(|run| run.metrics())
            .fold((0.0, 0), |(sum, count), m| (sum + m.cpu_usage, count + 1));

        let avg_cpu_utilization: f64 = if total_metrics > 0 {
            total_cpu_usage / total_metrics as f64
        } else {
            0.0
        };
        let last_start_time: u64 = last_runs
            .iter()
            .map(|run| run.iteration().start_time)
            .max()
            .unwrap_or(0) as u64;

        let co2_emission_trend: Vec<f64> = (1..=10).map(|x| x as f64).collect(); // Placeholder

        let runs: Vec<ScenarioRun> = scenario_map
            .into_iter()
            .map(|(run_id, iterations)| ScenarioRun { run_id, iterations })
            .collect();

        scenario_responses.push(Scenario5Average {
            name: scenario_name,
            avg_co2_emission,
            last_5_avg_cpu: avg_cpu_utilization,
            avg_power_consumption,
            co2_emission_trend,
            last_start_time,
            runs,
        });
    }

    // Sort by scenario last time
    scenario_responses.sort_by(|a, b| b.last_start_time.cmp(&a.last_start_time));

    let pagination = Pagination {
        current_page: page + 1, // Page is DB value which uses 0 based indexing, user needs 1 based
        // indexing
        per_page: limit,
        total_scenarios,
        total_pages,
    };

    let response = ScenariosResponse {
        scenarios: scenario_responses,
        pagination,
    };

    debug!(
        "Returning response with {} scenarios",
        response.scenarios.len()
    );
    Ok(Json(response))
}

#[instrument(name = "Get specific scenario")]
pub async fn get_scenario(
    State(db): State<DatabaseConnection>,
    Path(scenario_id): Path<String>,
    Query(params): Query<ScenarioParams>,
) -> Result<Json<ScenarioResponse>, ServerError> {
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(5);

    // Fetch all unique run_ids for the scenario
    let all_run_ids = dao::iteration::fetch_unique_run_ids(&scenario_id, &db).await?;

    // Calculate pagination
    let total_runs = all_run_ids.len();
    let total_pages = (total_runs as f64 / limit as f64).ceil() as u32;
    let start_index = (page - 1) as usize * limit as usize;
    let end_index = (start_index + limit as usize).min(total_runs);

    // Paginate run_ids
    let paginated_run_ids = &all_run_ids[start_index..end_index];

    let mut scenario_runs = Vec::new();
    let mut total_cpu_utilization = 0.0;
    let mut total_iterations = 0;
    let mut last_start_time = 0u64;

    for RunId { run_id } in paginated_run_ids {
        let iterations = dao::iteration::fetch_by_scenario_and_run(&scenario_id, *run_id, &db)
            .await?;

        let mut run_iterations = Vec::new();
        for iteration in iterations {
            let metrics = dao::metrics::fetch_within(*run_id, iteration.start_time, iteration.stop_time, &db)
                .await?;

            let usages: Vec<Usage> = metrics
                .iter()
                .map(|m| Usage {
                    cpu_usage: m.cpu_usage,
                    timestamp: m.time_stamp,
                })
                .collect();

            total_cpu_utilization += usages.iter().map(|u| u.cpu_usage).sum::<f64>();
            total_iterations += usages.len();
            last_start_time = last_start_time.max(iteration.start_time as u64);

            run_iterations.push(Iteration {
                run_id: iteration.run_id,
                scenario_name: iteration.scenario_name.clone(),
                iteration: iteration.count,
                start_time: iteration.start_time,
                stop_time: iteration.stop_time,
                usage: Some(usages),
            });
        }

        scenario_runs.push(ScenarioRun {
            run_id: *run_id,
            iterations: run_iterations,
        });
    }

    let avg_cpu_utilization = if total_iterations > 0 {
        total_cpu_utilization / total_iterations as f64
    } else {
        0.0
    };

    let scenario_response = ScenarioResponse {
        scenario: Scenario {
            name: scenario_id,
            avg_co2_emission: 0.0, // Placeholder value
            avg_cpu_utilization,
            avg_power_consumption: 0.0,     // Placeholder value
            co2_emission_trend: Vec::new(), // Fill this if you have the data
            last_start_time,
            runs: scenario_runs,
        },
        pagination: Pagination {
            current_page: page,
            total_pages,
            per_page: limit,
            total_scenarios: total_runs as u32,
        },
    };

    debug!(
        "Returning scenario response with {} runs",
        scenario_response.scenario.runs.len()
    );
    Ok(Json(scenario_response))
}

// #[instrument(name = "Get database url")]
// pub async fn get_database_url() -> Result<Json<String>, ServerError> {
//     //TODO change this to NOT include the password
//     let db_url = std::env::var("DATABASE_URL").unwrap();
//     Ok(Json(db_url))
// }
