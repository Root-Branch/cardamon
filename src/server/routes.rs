use crate::{
    dataset::DatasetBuilder,
    models,
    server::{
        errors::ServerError,
        types::{
            Pagination, ProcessData, RunData, ScenarioData, ScenariosParams, ScenariosResponse,
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

    let mut scenario_data: Vec<ScenarioData> = vec![];
    for sc_ds in dataset.by_scenario() {
        let scenario_name = sc_ds.scenario_name().to_string();

        let mut run_data: Vec<RunData> = vec![];
        for sc_run_ds in sc_ds.by_run() {
            let run_id = sc_run_ds.run_id();

            let mut run_pow = 0_f64;
            let mut run_co2 = 0_f64;
            let mut proc_data: Vec<ProcessData> = vec![];
            let sc_run_its_ds = sc_run_ds.by_iteration();
            for sc_run_it_ds in sc_run_its_ds {
                let mut it_pow = 0_f64;
                let mut it_co2 = 0_f64;

                for (proc_id, metrics) in sc_run_it_ds.by_process() {
                    let cardamon_data_for_proc =
                        models::rab_linear_model(metrics, 42_f64, 1337_f64);
                    let pow = cardamon_data_for_proc.pow;
                    let co2 = cardamon_data_for_proc.co2;

                    it_pow += pow;
                    it_co2 += co2;
                    proc_data.push(ProcessData {
                        proc_id,
                        pow,
                        co2,
                        pow_perc: 0_f64,
                    });
                }

                run_pow += it_pow;
                run_co2 += it_co2;

                // calculate pow perc for each process
                for proc_data in proc_data.iter_mut() {
                    proc_data.pow_perc = proc_data.pow / it_pow;
                }
            }
            let it_count = sc_run_its_ds.len() as f64;
            run_pow /= it_count;
            run_co2 /= it_count;

            run_data.push(RunData {
                run_id,
                run_pow,
                run_co2,
                proc_data,
            })
        }

        // calculate trend
        let mut delta_sum = 0_f64;
        let mut delta_sum_abs = 0_f64;
        for i in 0..run_data.len() - 1 {
            let delta = run_data[i + 1].run_pow - run_data[i].run_pow;
            delta_sum += delta;
            delta_sum_abs += delta.abs();
        }

        scenario_data.push(ScenarioData {
            scenario_name,
            run_data,
            trend: delta_sum / delta_sum_abs,
        })
    }

    Ok(Json(ScenariosResponse {
        scenario_data,
        pagination: Pagination {
            current_page: page,
            per_page: limit,
            total_pages: dataset.total_scenarios / limit,
        },
    }))
}
