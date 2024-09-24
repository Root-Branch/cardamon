use crate::{
    dao::pagination::Pages,
    data::{
        dataset::{AggregationMethod, Dataset, LiveDataFilter},
        dataset_builder::DatasetBuilder,
        ProcessMetrics, ScenarioData,
    },
    models::{self, rab_linear_model},
    server::errors::ServerError,
};
use anyhow::Context;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use itertools::Itertools;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub current_page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenariosParams {
    pub from_date: Option<i64>,
    pub to_date: Option<i64>,
    pub search_query: Option<String>,
    pub last_n: Option<u64>,
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResponse {
    pub scenario_name: String,
    pub last_run: i64,
    pub pow: f64,
    pub co2: f64,
    pub sparkline: Vec<f64>,
    pub trend: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenariosResponse {
    pub scenarios: Vec<ScenarioResponse>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunsParams {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResponse {
    pub process_name: String,
    pub pow_contrib_perc: f64,
    pub iteration_metrics: Vec<Vec<ProcessMetrics>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    pub start_time: i64,
    pub duration: f64,
    pub pow: f64,
    pub co2: f64,
    pub processes: Vec<ProcessResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunsResponse {
    pub runs: Vec<RunResponse>,
    pub pagination: Pagination,
}

pub async fn build_scenario_data(
    dataset: &Dataset,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioData>> {
    let mut scenario_data = vec![];
    for scenario_dataset in dataset.by_scenario(LiveDataFilter::IncludeLive) {
        let data = scenario_dataset
            .apply_model(
                &db,
                &models::rab_linear_model(0.12),
                AggregationMethod::MostRecent,
            )
            .await?;
        scenario_data.push(data);
    }

    Ok(scenario_data)
}

#[instrument(name = "Get list of scenarios")]
pub async fn get_scenarios(
    State(db): State<DatabaseConnection>,
    Query(params): Query<ScenariosParams>,
) -> Result<Json<ScenariosResponse>, ServerError> {
    let begin = params.from_date.unwrap_or(0);
    let end = params
        .to_date
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let last_n = params.last_n.unwrap_or(5);
    let page = params.page.unwrap_or(1);
    let page = page - 1; // DB needs -1 indexing
    let limit = params.limit.unwrap_or(5);

    info!("Fetching scenarios between {} and {}", begin, end);

    let dataset = match &params.search_query {
        Some(query) => {
            DatasetBuilder::new()
                .scenarios_by_name(query)
                .page(limit, page)
                .last_n_runs(last_n)
                .all()
                .build(&db)
                .await?
        }
        None => {
            DatasetBuilder::new()
                .scenarios_in_range(begin, end)
                .page(limit, page)
                .last_n_runs(last_n)
                .all()
                .build(&db)
                .await?
        }
    };

    let scenario_data = build_scenario_data(&dataset, &db).await?;
    let total_pages = match dataset.total_scenarios {
        Pages::NotRequired => 0,
        Pages::Required(pages) => pages,
    };

    let mut scenarios = vec![];
    for scenario_data in scenario_data {
        let scenario_name = scenario_data.scenario_name;
        let last_run = scenario_data.run_data.first().context("")?.start_time;
        let pow = scenario_data.data.pow;
        let co2 = scenario_data.data.co2;
        let sparkline = scenario_data
            .run_data
            .iter()
            .map(|run_data| run_data.data.pow)
            .collect_vec();
        let trend = scenario_data.trend;

        scenarios.push(ScenarioResponse {
            scenario_name,
            last_run,
            pow,
            co2,
            sparkline,
            trend,
        });
    }

    Ok(Json(ScenariosResponse {
        scenarios,
        pagination: Pagination {
            current_page: page + 1,
            per_page: limit,
            total_pages,
        },
    }))
}

pub async fn get_runs(
    State(db): State<DatabaseConnection>,
    Path(scenario_name): Path<String>,
    Query(params): Query<RunsParams>,
) -> Result<Json<RunsResponse>, ServerError> {
    let page = params.page.unwrap_or(1);
    let page = page - 1; // DB needs -1 indexing
    let limit = params.limit.unwrap_or(5);

    info!("Fetching runs for scenario with name {} ", scenario_name);

    let dataset = DatasetBuilder::new()
        .scenario(&scenario_name)
        .all()
        .runs_all()
        .page(limit, page)?
        .build(&db)
        .await?;
    let total_pages = match dataset.total_runs {
        Pages::NotRequired => 0,
        Pages::Required(pages) => pages,
    };

    let mut runs = vec![];
    for scenario_dataset in &dataset.by_scenario(LiveDataFilter::IncludeLive) {
        for run_dataset in scenario_dataset.by_run() {
            let model_data = run_dataset
                .apply_model(&db, &rab_linear_model(0.12))
                .await?;
            let processes = model_data
                .process_data
                .iter()
                .map(|data| ProcessResponse {
                    process_name: data.process_id.clone(),
                    pow_contrib_perc: data.pow_perc,
                    iteration_metrics: data.iteration_metrics.clone(),
                })
                .collect_vec();

            runs.push(RunResponse {
                start_time: model_data.start_time,
                duration: model_data.duration(),
                pow: model_data.data.pow,
                co2: model_data.data.co2,
                processes,
            });
        }
    }

    Ok(Json(RunsResponse {
        runs,
        pagination: Pagination {
            current_page: page + 1,
            per_page: limit,
            total_pages,
        },
    }))
}

#[cfg(test)]
mod tests {
    use crate::{
        data::dataset_builder::DatasetBuilder, db_connect, db_migrate,
        server::routes::build_scenario_data, tests::setup_fixtures,
    };

    #[tokio::test]
    async fn building_data_response_for_ui_should_work() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .all()
            .build(&db)
            .await?;

        let _res = build_scenario_data(&dataset, &db).await?;

        // uncomment to see generated json response
        let json_str = serde_json::to_string_pretty(&_res)?;
        println!("{}", json_str);

        Ok(())
    }
}
