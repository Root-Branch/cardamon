use super::errors::ServerError;
use axum::{
    extract::{Query, State},
    Json,
};
use cardamon::data_access::{iteration::Iteration, pagination::Page, DAOService, LocalDAOService};
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct AllParams {
    scenario: String,
    page_size: u32,
    page_num: u32,
}

#[derive(Debug, Deserialize)]
pub struct InRangeParams {
    scenario: String,
    from: i64,
    to: i64,
    page_size: u32,
    page_num: u32,
}

#[derive(Debug, Deserialize)]
pub struct LastNParams {
    scenario: String,
    last_n: u32,
}

#[instrument(name = "Fetch iterations for all runs")]
pub async fn fetch_runs_all(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<AllParams>,
) -> Result<Json<Vec<Iteration>>, ServerError> {
    let scenario = params.scenario;
    let page = Page::new(params.page_size, params.page_num);

    tracing::debug!(
        "Received request to fetch all iterations for scenario: {}, page number: {}, page size: {}",
        scenario,
        page.size,
        page.num
    );
    let iterations = dao_service
        .iterations()
        .fetch_runs_all(&scenario, &page)
        .await?;

    tracing::info!(
        "Successfully fetched {} iterations",
        iterations.len()
    );
    Ok(Json(iterations))
}

#[instrument(name = "Fetch iterations for the given scenario in the given range")]
pub async fn fetch_runs_in_range(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<InRangeParams>,
) -> Result<Json<Vec<Iteration>>, ServerError> {
    let scenario = params.scenario;
    let from = params.from;
    let to = params.to;
    let page = Page::new(params.page_size, params.page_num);

    tracing::debug!(
        "Received request to fetch all iterations for scenario in range: {}, from: {}, to: {}, page number: {}, page size: {}",
        scenario,
        from, 
        to,
        page.size,
        page.num
    );
    let iterations = dao_service
        .iterations()
        .fetch_runs_in_range(&scenario, from, to, &page)
        .await?;

    tracing::info!(
        "Successfully fetched {} iterations",
        iterations.len()
    );
    Ok(Json(iterations))
}

#[instrument(name = "Fetch last 'n' iterations for the given scenario")]
pub async fn fetch_runs_last_n(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<LastNParams>,
) -> Result<Json<Vec<Iteration>>, ServerError> {
    let scenario = params.scenario;
    let last_n = params.last_n;

    tracing::debug!(
        "Received request to fetch last n iterations for scenario: {}, last_n: {}",
        scenario,
        last_n
    );
    let iterations = dao_service
        .iterations()
        .fetch_runs_last_n(&scenario, last_n)
        .await?;

    tracing::info!(
        "Successfully fetched {} iterations",
        iterations.len()
    );
    Ok(Json(iterations))
}

#[instrument(name = "Persist iteration into database")]
pub async fn persist(
    State(dao_service): State<LocalDAOService>,
    Json(payload): Json<Iteration>,
) -> Result<String, ServerError> {
    tracing::debug!("Received payload: {:?}", payload);
    dao_service.iterations().persist(&payload).await?;

    tracing::info!("Iteration persisted successfully");
    Ok("Iteration persisted".to_string())
}
