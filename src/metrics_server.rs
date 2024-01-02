/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub mod dao;
pub mod dao_schema;
pub mod dto;

use crate::metrics_server::dao::{GetCpuMetrics, GetScenario};

use axum::{
    extract,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use diesel::{prelude::*, RunQueryDsl, SqliteConnection};
use itertools::Itertools;
use log::info;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

type Db = Arc<Mutex<SqliteConnection>>;

async fn insert_scenario_run(
    extract::State(db): extract::State<Db>,
    extract::Json(body): extract::Json<dto::Scenario>,
) {
    let mut db_conn = db.lock().unwrap();

    let new_scenario = dao::NewScenario {
        scenario: dao::Scenario {
            cardamon_run_type: body.cardamon_run_type,
            cardamon_run_id: body.cardamon_run_id,
            scenario_name: body.scenario_name,
            start_time: NaiveDateTime::from_timestamp_millis(body.start_time).unwrap(),
            stop_time: NaiveDateTime::from_timestamp_millis(body.stop_time).unwrap(),
        },
    };

    diesel::insert_into(dao_schema::scenario::table)
        .values(&new_scenario)
        .returning(dao::Scenario::as_returning())
        .get_result(&mut *db_conn)
        .expect("Error saving new scenario");
}

async fn insert_metrics(
    extract::State(db): extract::State<Db>,
    extract::Json(body): extract::Json<dto::Batch>,
) {
    let mut db_conn = db.lock().unwrap();

    for m in body.metrics {
        match m {
            dto::Metrics::DockerContainerCpu {
                timestamp,
                fields,
                tags,
            } => {
                // TODO: This needs to be redone! Should we join all these futures together? What happens if one fails,
                // does it cause all subsequent futures to fail? We basically want to log failure and carry on to the next
                // future because it doesn't really matter if we lose the odd metric.
                let cpu_metrics = dao::NewCpuMetrics {
                    metrics: dao::CpuMetrics {
                        cardamon_run_type: tags.cardamon_run_type,
                        cardamon_run_id: tags.cardamon_run_id,
                        container_name: tags.container_name,
                        container_id: fields.container_id,
                        throttling_periods: fields.throttling_periods,
                        throttling_throttled_periods: fields.throttling_throttled_periods,
                        throttling_throttled_time: fields.throttling_throttled_time,
                        usage_in_kernelmode: fields.usage_in_kernelmode,
                        usage_in_usermode: fields.usage_in_usermode,
                        usage_percent: fields.usage_percent,
                        usage_system: fields.usage_system,
                        usage_total: fields.usage_total,
                        timestamp: NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap(),
                    },
                };

                diesel::insert_into(dao_schema::cpu_metrics::table)
                    .values(&cpu_metrics)
                    .execute(&mut *db_conn)
                    .expect("Error saving cpu metrics");
            }

            dto::Metrics::DockerContainerMem {
                timestamp: _,
                fields: _,
                tags: _,
            } => {}
        };
    }
}

fn create_process_stats(
    process_name: String,
    metrics: Vec<GetCpuMetrics>,
    tdp_w: f64,
) -> dto::ProcessStats {
    struct MetricSlice {
        value: f64,
        dt_ms: i64,
    }

    let slices = metrics
        .iter()
        .tuple_windows()
        .map(|(a, b)| MetricSlice {
            value: a.metrics.usage_percent,
            dt_ms: b.metrics.timestamp.timestamp_millis() - a.metrics.timestamp.timestamp_millis(),
        })
        .collect::<Vec<_>>();

    let energy_consumption_w: f64 = slices.iter().fold(0.0, |acc, b| {
        acc + b.value * (b.dt_ms as f64 * tdp_w) / 1000.0
    });

    dto::ProcessStats {
        process_name,
        energy_consumption_w,
    }
}

async fn create_summary(
    extract::State(db): extract::State<Db>,
    extract::Json(body): extract::Json<dto::ScenarioSummaryOpts>,
) -> Json<Vec<dto::ScenarioRunStats>> {
    use crate::metrics_server::dao_schema::cpu_metrics::dsl as m_dsl;
    use crate::metrics_server::dao_schema::scenario::dsl as sc_dsl;

    let mut db_conn = db.lock().unwrap();

    // for each scenario that was run, build a summary of the energy used and carbon emitted per process
    let mut stats: Vec<dto::ScenarioRunStats> = vec![];
    for scenario_name in body.scenarios {
        // take the most recent n scenario runs
        let scenarios: Vec<GetScenario> = sc_dsl::scenario
            .filter(sc_dsl::scenario_name.eq(scenario_name.clone()))
            .order_by(sc_dsl::start_time.desc())
            .limit(body.last_n as i64)
            .select(GetScenario::as_select())
            .load(&mut *db_conn)
            .expect("Error reading scenarios!");

        // for each scenario run grab the metrics for that run and group them by process
        let mut run_stats: Vec<dto::RunStats> = vec![];
        for run in scenarios {
            // get metrics for this scenario run
            let metrics: Vec<GetCpuMetrics> = m_dsl::cpu_metrics
                .filter(m_dsl::cardamon_run_id.eq(&run.scenario.cardamon_run_id))
                .filter(m_dsl::timestamp.between(&run.scenario.start_time, &run.scenario.stop_time))
                .order_by(m_dsl::timestamp.asc())
                .select(GetCpuMetrics::as_select())
                .load(&mut *db_conn)
                .expect("Error reading cpu metrics");

            // group metrics by process_name and produce ProcessStats from metrics
            let process_stats = metrics
                .into_iter()
                .group_by(|x| x.metrics.container_name.clone())
                .into_iter()
                .map(|(process_name, metrics)| {
                    create_process_stats(process_name, metrics.collect_vec(), body.cpu_tdp)
                })
                .collect::<Vec<_>>();

            run_stats.push(dto::RunStats {
                start_time: run.scenario.start_time,
                process_stats,
            });
        }

        stats.push(dto::ScenarioRunStats {
            scenario_name,
            run_stats,
        });
    }

    axum::Json(stats)
}

pub fn start(db: Db) -> JoinHandle<()> {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/metrics", post(insert_metrics))
            .route("/scenario", post(insert_scenario_run))
            .route("/scenario_summary", get(create_summary))
            .with_state(db);

        // run our app with hyper, listening globally on port 2050
        info!("Starting server on localhost:2050");
        let listener = tokio::net::TcpListener::bind("localhost:2050")
            .await
            .unwrap();
        axum::serve(listener, app).await.unwrap();
    })
}
