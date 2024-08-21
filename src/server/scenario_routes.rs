use super::errors::ServerError;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cardamon::data_access::{pagination::Page, DAOService, LocalDAOService};
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct AllParams {
    page_size: Option<u32>,
    page_num: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct InRunParams {
    run: String,
    page_size: Option<u32>,
    page_num: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct InRangeParams {
    from: i64,
    to: i64,
    page_size: Option<u32>,
    page_num: Option<u32>,
}

#[instrument(name = "Fetch all scenarios")]
pub async fn fetch_all(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<AllParams>,
) -> Result<Json<Vec<String>>, ServerError> {
    let page = if params.page_size.is_some() && params.page_num.is_some() {
        Some(Page::new(
            params.page_size.unwrap(),
            params.page_num.unwrap(),
        ))
    } else {
        None
    };

    tracing::debug!("Received request to fetch all scenarios");
    let scenarios = dao_service.scenarios().fetch_all(&page).await?;

    tracing::info!("Successfully fetched {} iterations", scenarios.len());
    Ok(Json(scenarios))
}

#[instrument(name = "Fetch all scenarios for the given run")]
pub async fn fetch_in_run(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<InRunParams>,
) -> Result<Json<Vec<String>>, ServerError> {
    let run = params.run;
    let page = if params.page_size.is_some() && params.page_num.is_some() {
        Some(Page::new(
            params.page_size.unwrap(),
            params.page_num.unwrap(),
        ))
    } else {
        None
    };

    tracing::debug!(
        "Received request to fetch all scenarios for the given run: run: {}",
        run,
    );
    let scenarios = dao_service.scenarios().fetch_in_run(&run, &page).await?;

    tracing::info!("Successfully fetched {} iterations", scenarios.len());
    Ok(Json(scenarios))
}

#[instrument(name = "Fetch scenarios run in the given range")]
pub async fn fetch_in_range(
    State(dao_service): State<LocalDAOService>,
    Query(params): Query<InRangeParams>,
) -> Result<Json<Vec<String>>, ServerError> {
    let from = params.from;
    let to = params.to;
    let page = if params.page_size.is_some() && params.page_num.is_some() {
        Some(Page::new(
            params.page_size.unwrap(),
            params.page_num.unwrap(),
        ))
    } else {
        None
    };

    tracing::debug!(
        "Received request to fetch all scenarios run in the given range: from {}, to {}",
        from,
        to
    );
    let scenarios = dao_service
        .scenarios()
        .fetch_in_range(from, to, &page)
        .await?;

    tracing::info!("Successfully fetched {} iterations", scenarios.len());
    Ok(Json(scenarios))
}

#[instrument(name = "Fetch scenarios by name")]
pub async fn fetch_by_name(
    State(dao_service): State<LocalDAOService>,
    Path(name): Path<String>,
    Query(params): Query<InRangeParams>,
) -> Result<Json<Vec<String>>, ServerError> {
    let page = if params.page_size.is_some() && params.page_num.is_some() {
        Some(Page::new(
            params.page_size.unwrap(),
            params.page_num.unwrap(),
        ))
    } else {
        None
    };

    tracing::debug!(
        "Received request to fetch all scenarios by name: name {}",
        name
    );
    let scenarios = dao_service.scenarios().fetch_by_name(&name, &page).await?;

    tracing::info!("Successfully fetched {} iterations", scenarios.len());
    Ok(Json(scenarios))
}
