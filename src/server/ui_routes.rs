use super::errors::ServerError;
use crate::server::ui_types::{
    ScenarioParams, ScenarioResponse, ScenariosParams, ScenariosResponse,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{pagination::Page, DAOService, LocalDataAccessService};
use chrono::Utc;
use tracing::instrument;

#[instrument(name = "Get list of scenarios")]
pub async fn get_scenarios(
    State(dao_service): State<LocalDataAccessService>,
    Query(params): Query<ScenariosParams>,
) -> Result<Json<ScenariosResponse>, ServerError> {
    let begin = params.from_date.unwrap_or(0);
    let end = params
        .to_date
        .unwrap_or_else(|| Utc::now().timestamp_millis().try_into().unwrap());
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(5);
    let pageinator = Some(Page::new(limit, page));
    let scenarios = dao_service
        .scenarios()
        .fetch_in_range(begin, end, &pageinator)
        .await;

    todo!("Implement get_scenarios")
}

#[instrument(name = "Get specific scenario")]
pub async fn get_scenario(
    State(dao_service): State<LocalDataAccessService>,
    Path(scenario_id): Path<String>,
    Query(params): Query<ScenarioParams>,
) -> Result<Json<ScenarioResponse>, ServerError> {
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(5);
    todo!("Implement get_scenario")
}

#[instrument(name = "Get database url")]
pub async fn get_database_url() -> Result<Json<String>, ServerError> {
    // Implement the logic to get the database URL
    todo!("Implement get_database_url")
}
