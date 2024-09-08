use std::collections::HashMap;

use crate::{
    dao,
    data::{self, dataset::Dataset, dataset_builder::DatasetBuilder},
    models::{self, rab_linear_model},
    server::{
        errors::ServerError,
        types::{
            Pagination, ProcessDataResponse, RunDataResponse, ScenarioDataResponse,
            ScenariosParams, ScenariosResponse,
        },
    },
};
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use tracing::{info, instrument};

pub async fn build_scenario_data(
    dataset: &Dataset,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioDataResponse>> {
    let f = rab_linear_model(42.0);
    for scenario_dataset in dataset.by_scenario() {
        let scenario_data = scenario_dataset.apply_model(db, &f);
    }

    todo!()
}

// pub async fn build_scenario_data(
//     dataset: &Dataset,
//     db: &DatabaseConnection,
// ) -> anyhow::Result<Vec<ScenarioDataResponse>> {
//     let mut scenario_data: Vec<ScenarioDataResponse> = vec![];
//     for scenario_dataset in dataset.by_scenario() {
//         let scenario_name = scenario_dataset.scenario_name().to_string();
//
//         let mut run_data: Vec<RunDataResponse> = vec![];
//         for scenario_run_dataset in scenario_dataset.by_run() {
//             let run_id = scenario_run_dataset.run_id();
//
//             // fetch cpu avgerage power
//             let cpu_avg_pow = dao::run::fetch(run_id, &db).await?.cpu_avg_power;
//
//             // fetch carbon intensity
//             let carbon_intensity = 23.0;
//
//             // build up process map
//             // proc_id  |  data for proc per iteration
//             // =======================================
//             // proc_id -> [<data>, <data>]             <- 2 iterations
//             // proc_id -> [<data>, <data>]
//             let mut proc_iteration_data_map: HashMap<String, Vec<data::Data>> = HashMap::new();
//             for scenario_run_iteration_dataset in scenario_run_dataset.by_iteration() {
//                 for (proc_id, metrics) in scenario_run_iteration_dataset.by_process() {
//                     // run the RAB model to get power and co2 emissions
//                     let cardamon_data =
//                         models::rab_linear_model(metrics, cpu_avg_pow, carbon_intensity);
//
//                     // if key already exists in map the append cardamon_data to the end of the
//                     // iteration data vector for that key, else create a new vector for that key.
//                     let data_vec = match proc_iteration_data_map.get_mut(&proc_id) {
//                         Some(data) => {
//                             let mut it_data = vec![];
//                             it_data.append(data);
//                             it_data.push(cardamon_data);
//                             it_data
//                         }
//
//                         None => vec![cardamon_data],
//                     };
//                     proc_iteration_data_map.insert(proc_id.to_string(), data_vec);
//                 }
//             }
//
//             // average data for each process across all iterations
//             let proc_data_map: HashMap<String, data::Data> = proc_iteration_data_map
//                 .iter()
//                 .map(|(k, v)| (k, v.iter().collect_vec()))
//                 .map(|(k, v)| (k.to_string(), data::Data::mean(&v)))
//                 .collect();
//
//             // calculate total run data (pow + co2)
//             let run_total = data::Data::sum(&proc_data_map.values().collect_vec());
//
//             // convert proc_data_map to vector of ProcessData
//             let proc_data = proc_data_map
//                 .into_iter()
//                 .map(|(proc_id, data)| ProcessDataResponse {
//                     proc_id,
//                     pow: data.pow,
//                     co2: data.co2,
//                     pow_perc: data.pow / run_total.pow,
//                 })
//                 .collect_vec();
//
//             run_data.push(RunDataResponse {
//                 run_id,
//                 run_pow: run_total.pow,
//                 run_co2: run_total.co2,
//                 proc_data,
//             })
//         }
//
//         // calculate trend
//         let mut delta_sum = 0_f64;
//         let mut delta_sum_abs = 0_f64;
//         for i in 0..run_data.len() - 1 {
//             let delta = run_data[i + 1].run_pow - run_data[i].run_pow;
//             delta_sum += delta;
//             delta_sum_abs += delta.abs();
//         }
//
//         scenario_data.push(ScenarioDataResponse {
//             scenario_name,
//             run_data,
//             trend: if delta_sum_abs != 0_f64 {
//                 delta_sum / delta_sum_abs
//             } else {
//                 0_f64
//             },
//         })
//     }
//
//     Ok(scenario_data)
// }

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
                .page(limit, page)
                .last_n_runs(5)
                .await?
        }
        None => {
            DatasetBuilder::new(&db)
                .scenarios_in_range(begin, end)
                .page(limit, page)
                .last_n_runs(5)
                .await?
        }
    };

    let scenario_data = build_scenario_data(&dataset, &db).await?;

    Ok(Json(ScenariosResponse {
        scenario_data,
        pagination: Pagination {
            current_page: page + 1,
            per_page: limit,
            total_pages: dataset.total_scenarios / limit,
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

        let dataset = DatasetBuilder::new(&db)
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .await?;

        let _res = build_scenario_data(&dataset, &db).await?;

        // uncomment to see generated json response
        // let json_str = serde_json::to_string_pretty(&_res)?;
        // println!("{}", json_str);

        Ok(())
    }
}
