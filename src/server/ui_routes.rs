use super::{
    errors::ServerError,
    ui_types::{Iteration, ScenarioRun, Usage},
};
use crate::server::ui_types::{
    Pagination, Scenario, ScenarioParams, ScenarioResponse, ScenariosParams, ScenariosResponse,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::LocalDAOService;
use cardamon::dataset::DatasetBuilder;
use chrono::Utc;
use std::collections::HashMap;
use tracing::instrument;
use tracing::{debug, info};

#[instrument(name = "Get list of scenarios")]
pub async fn get_scenarios(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<ScenariosParams>,
) -> Result<Json<ScenariosResponse>, ServerError> {
    let begin = params.from_date.unwrap_or(0);
    let end = params
        .to_date
        .unwrap_or_else(|| Utc::now().timestamp_millis().try_into().unwrap());
    let page = params.page.unwrap_or(0);
    let limit = params.limit.unwrap_or(5);
    let dataset = DatasetBuilder::new(&dao_service);

    info!("Fetching scenarios between {} and {}", begin, end);
    let scenarios = match params.search_query {
        Some(query) => {
            dataset
                .scenarios_by_name(&query)
                .page(limit, page)
                .last_n_runs(5)
                .await?
        }
        None => {
            dataset
                .scenarios_in_range(begin, end)
                .page(limit, page)
                .last_n_runs(5)
                .await?
        }
    };
    debug!("Fetched {} scenarios", scenarios.data().len());

    let mut scenario_map: HashMap<String, Vec<Iteration>> = HashMap::new();
    for scenario in scenarios.data().iter() {
        scenario_map
            .entry(scenario.iteration().scenario_name.clone())
            .or_insert_with(Vec::new)
            .push(Iteration {
                run_id: scenario.iteration().run_id.clone(),
                scenario_name: scenario.iteration().scenario_name.clone(),
                iteration: scenario.iteration().iteration,
                start_time: scenario.iteration().start_time,
                stop_time: scenario.iteration().stop_time,
                usage: None,
            });
    }

    let mut scenario_responses = Vec::new();
    for (name, iterations) in scenario_map.iter() {
        let avg_co2_emission: f64 = 2.0; // Placeholder value
        let avg_power_consumption: f64 = 2.0; // Placeholder value
        let avg_cpu_utilization: f64 = 0.0; // Placeholder value, should calculate based on iterations

        let last_start_time: u64 =
            iterations.iter().map(|i| i.start_time).max().unwrap_or(0) as u64;
        let co2_emission_trend: Vec<f64> = (1..=10).map(|x| x as f64).collect();

        let runs = iterations
            .iter()
            .map(|i| ScenarioRun {
                run_id: i.run_id.clone(),
                iterations: vec![i.clone()],
            })
            .collect();

        scenario_responses.push(Scenario {
            name: name.clone(),
            avg_co2_emission,
            avg_cpu_utilization,
            avg_power_consumption,
            co2_emission_trend,
            last_start_time,
            runs,
        });
    }

    // Sort by scenario last time
    scenario_responses.sort_by(|a, b| b.last_start_time.cmp(&a.last_start_time));

    let total_scenarios = scenario_responses.len() as f64;
    let total_pages = 0; // TODO ADD PAGES

    let pagination = Pagination {
        current_page: page,
        per_page: limit,
        total_scenarios: total_scenarios as u32,
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
    State(dao_service): State<LocalDAOService>,
    Path(scenario_id): Path<String>,
    Query(params): Query<ScenarioParams>,
) -> Result<Json<ScenarioResponse>, ServerError> {
    let page = params.page.unwrap_or(0);
    let limit = params.limit.unwrap_or(5);
    let dataset = DatasetBuilder::new(&dao_service);
    let scenario_data = dataset
        .scenarios_by_name(&scenario_id)
        .page(limit, page)
        .last_n_runs(5)
        .await?;

    if scenario_data.data().is_empty() {
        return Err(ServerError::NotFound("Scenario not found".into()));
    }

    let mut total_cpu_utilization: f64 = 0.0;
    let mut scenario_map: HashMap<String, Vec<Iteration>> = HashMap::new();
    for scenario in scenario_data.data().iter() {
        let mut usages: Vec<Usage> = scenario
            .metrics()
            .iter()
            .map(|m| Usage {
                cpu_usage: m.cpu_usage,
                timestamp: m.time_stamp,
            })
            .collect();

        usages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Sort usages by timestamp descending

        scenario_map
            .entry(scenario.iteration().run_id.clone())
            .or_insert_with(Vec::new)
            .push(Iteration {
                run_id: scenario.iteration().run_id.clone(),
                scenario_name: scenario.iteration().scenario_name.clone(),
                iteration: scenario.iteration().iteration,
                start_time: scenario.iteration().start_time,
                stop_time: scenario.iteration().stop_time,
                usage: Some(usages),
            });

        total_cpu_utilization += scenario.metrics().iter().map(|m| m.cpu_usage).sum::<f64>();
    }

    let scenario_name = scenario_data
        .data()
        .first()
        .unwrap()
        .iteration()
        .scenario_name
        .clone();
    let last_start_time = scenario_data
        .data()
        .last()
        .map(|s| s.iteration().start_time as u64)
        .unwrap_or(0);

    let runs = scenario_map
        .iter()
        .map(|(run_id, iterations)| ScenarioRun {
            run_id: run_id.clone(),
            iterations: iterations.clone(),
        })
        .collect();

    let scenario_response = ScenarioResponse {
        scenario: Scenario {
            name: scenario_name,
            avg_co2_emission: 0.0, // Placeholder value
            avg_cpu_utilization: total_cpu_utilization / scenario_data.data().len() as f64,
            avg_power_consumption: 0.0,     // Placeholder value
            co2_emission_trend: Vec::new(), // Fill this if you have the data
            last_start_time,
            runs,
        },
        pagination: Pagination {
            current_page: page,
            total_pages: 0,
            per_page: limit,
            total_scenarios: scenario_data.data().len() as u32,
        },
    };

    Ok(Json(scenario_response))
}

#[instrument(name = "Get database url")]
pub async fn get_database_url() -> Result<Json<String>, ServerError> {
    //TODO change this to NOT include the password
    let db_url = std::env::var("DATABASE_URL").unwrap();
    Ok(Json(db_url))
}
