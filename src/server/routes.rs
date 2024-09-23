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
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
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
pub struct ScenariosResponse {
    pub scenario_data: Vec<ScenarioData>,
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

    Ok(Json(ScenariosResponse {
        scenario_data,
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
