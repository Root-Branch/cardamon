use super::errors::ServerError;
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
use sqlx::SqlitePool;
use tracing::instrument;
use tracing::{debug, info};

#[instrument(name = "Get list of scenarios")]
pub async fn get_scenarios(
    State(dao_service): State<LocalDAOService>,
    State(pool): State<SqlitePool>,
    Query(params): Query<ScenariosParams>,
) -> Result<Json<ScenariosResponse>, ServerError> {
    let begin = params.from_date.unwrap_or(0);
    let end = params
        .to_date
        .unwrap_or_else(|| Utc::now().timestamp_millis().try_into().unwrap());
    let page = params.page.unwrap_or(0);
    let limit = params.limit.unwrap_or(5);
    let ds = LocalDAOService::new(pool);
    let dataset = DatasetBuilder::new(&ds);

    info!("Fetching scenarios between {} and {}", begin, end);
    let scenarios = dataset
        .scenarios_in_range(begin, end)
        .page(limit, page)
        .last_n_runs(5)
        .await?;

    debug!("Fetched {} scenarios", scenarios.data().len());

    let mut scenario_responses = Vec::new();
    for scenario in scenarios.data().iter() {
        let avg_co2_emission: f64 = 2.0; // Placeholder value
        let avg_power_consumption: f64 = 2.0; // Placeholder value
        let mets = scenario.metrics();
        let avg_cpu_utilization: f64 = if !mets.is_empty() {
            mets.iter().map(|m| m.cpu_usage).sum::<f64>() / mets.len() as f64
        } else {
            0.0
        };
        let last_start_time: u64 = mets
            .iter()
            .map(|m| m.time_stamp)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0) as u64;
        let co2_emission_trend: Vec<f64> = (1..=10).map(|x| x as f64).collect();
        scenario_responses.push(Scenario {
            name: scenario.iteration().scenario_name.clone(),
            avg_co2_emission,
            avg_cpu_utilization,
            avg_power_consumption,
            co2_emission_trend,
            last_start_time,
        });
    }
    //need this field
    //let total_scenarios = scenarios.total_count();
    let total_scenarios = 0;
    let total_pages = (total_scenarios as f64 / limit as f64).ceil() as u32;

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
    let _page = params.page.unwrap_or(1);
    let _limit = params.limit.unwrap_or(5);
    todo!("Implement get_scenario")
}

#[instrument(name = "Get database url")]
pub async fn get_database_url() -> Result<Json<String>, ServerError> {
    //TODO change this to NOT include the password
    let db_url = std::env::var("DATABASE_URL").unwrap();
    Ok(Json(db_url))
}
