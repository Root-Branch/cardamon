use crate::{
    dao::pagination::Pages,
    data::{
        dataset::{AggregationMethod, Dataset},
        dataset_builder::DatasetBuilder,
        ScenarioData,
    },
    models,
    server::errors::ServerError,
};
use anyhow::Context;
use axum::{
    extract::{Query, State},
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

pub async fn build_scenario_data(
    dataset: &Dataset,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioData>> {
    let mut scenario_data = vec![];
    for scenario_dataset in dataset.by_scenario(false) {
        let data = scenario_dataset
            .apply_model(
                &db,
                &models::rab_linear_model(42.0),
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
