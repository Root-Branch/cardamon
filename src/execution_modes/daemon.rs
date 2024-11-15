use crate::{
    entities, execution_plan::ProcessToObserve, metrics_logger, server::errors::ServerError,
};
use anyhow::Context;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use colored::Colorize;
use sea_orm::*;
use serde::{self, Deserialize};
use tokio::sync::mpsc;

enum Signal {
    Start { run_id: String },
    Stop,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartParams {
    pub run_id: String,
}

async fn start(
    State(tx): State<mpsc::Sender<Signal>>,
    Query(params): Query<StartParams>,
) -> Result<String, ServerError> {
    println!("start received {}", params.run_id);
    tx.send(Signal::Start {
        run_id: params.run_id,
    })
    .await
    .context("")?;

    Ok("success".to_string())
}

async fn stop(State(tx): State<mpsc::Sender<Signal>>) -> Result<String, ServerError> {
    println!("stop received");
    tx.send(Signal::Stop).await.context("")?;

    Ok("success".to_string())
}

pub async fn run_daemon(
    cpu_id: i32,
    region: &Option<String>,
    ci: f64,
    processes_to_observe: Vec<ProcessToObserve>,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel::<Signal>(10);

    let db = db.clone();
    let region = region.clone();

    tokio::spawn(async move {
        loop {
            let mut run_id: String = "".to_string();

            // wait for signal to start recording
            while let Some(signal) = rx.recv().await {
                if let Signal::Start { run_id: id } = signal {
                    run_id = id;
                    println!("Starting {}", run_id);
                    break;
                }
            }

            let start_time = Utc::now().timestamp_millis();

            // upsert run
            let txn = db.begin().await.unwrap();
            let active_run = entities::run::Entity::find_by_id(&run_id)
                .one(&txn)
                .await
                .unwrap();

            let mut active_run = if active_run.is_none() {
                entities::run::ActiveModel {
                    id: ActiveValue::Set(run_id.clone()),
                    is_live: ActiveValue::Set(true),
                    cpu_id: ActiveValue::Set(cpu_id),
                    region: ActiveValue::Set(region.clone()),
                    carbon_intensity: ActiveValue::Set(ci),
                    start_time: ActiveValue::Set(start_time),
                    stop_time: ActiveValue::set(None),
                }
                .insert(&txn)
                .await
                .unwrap()
                .into_active_model()
            } else {
                // this unwrap is safe
                active_run.unwrap().into_active_model()
            };

            // upsert iteration
            let active_iteration = entities::iteration::Entity
                .select()
                .filter(entities::iteration::Column::RunId.eq(&run_id))
                .one(&txn)
                .await
                .unwrap();
            let mut active_iteration = if active_iteration.is_none() {
                entities::iteration::ActiveModel {
                    id: ActiveValue::NotSet,
                    run_id: ActiveValue::Set(run_id.clone()),
                    scenario_name: ActiveValue::Set("live".to_string()),
                    count: ActiveValue::Set(1),
                    start_time: ActiveValue::Set(start_time),
                    stop_time: ActiveValue::Set(None), // same as start for now, will be updated later
                }
                .save(&txn)
                .await
                .unwrap()
            } else {
                active_iteration.unwrap().into_active_model()
            };

            txn.commit().await.unwrap();

            // start metric logger
            let stop_handle = metrics_logger::start_logging(
                processes_to_observe.clone(),
                run_id.clone(),
                db.clone(),
            )
            .unwrap(); // TODO: remove unwrap!

            // wait until stop signal is received
            while let Some(signal) = rx.recv().await {
                if let Signal::Stop = signal {
                    println!("Stopping!");
                    break;
                }
            }

            stop_handle.stop().await;

            // update the iteration stop time
            let now = Utc::now().timestamp_millis();
            active_iteration.stop_time = ActiveValue::Set(Some(now));
            active_iteration.update(&db).await.unwrap();

            // update the run stop time
            active_run.stop_time = ActiveValue::Set(Some(now));
            active_run.clone().update(&db).await.unwrap(); // TODO: remove unwrap!
        }
    });

    // start signal server
    let app = Router::new()
        .route("/start", get(start))
        .route("/stop", get(stop))
        .with_state(tx.clone());

    // Start the Axum server
    println!(
        "> waiting for start/stop signals on {}",
        "http://localhost:3030".green()
    );
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", 3030))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();

    Ok(())
}
