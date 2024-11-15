use crate::{
    entities::{iteration, run},
    execution_plan::ProcessToObserve,
    metrics_logger,
};
use chrono::Utc;
use nanoid::nanoid;
use sea_orm::*;

pub async fn run_live<'a>(
    cpu_id: i32,
    region: &Option<String>,
    ci: f64,
    processes_to_observe: Vec<ProcessToObserve>,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    let start_time = Utc::now().timestamp_millis();

    // create a new run
    let guid = nanoid!(5, &nanoid::alphabet::SAFE);
    let active_run = run::ActiveModel {
        id: ActiveValue::Set(guid),
        is_live: ActiveValue::Set(true),
        cpu_id: ActiveValue::Set(cpu_id),
        region: ActiveValue::Set(region.clone()),
        carbon_intensity: ActiveValue::Set(ci),
        start_time: ActiveValue::Set(start_time),
        stop_time: ActiveValue::set(None),
    }
    .insert(db)
    .await?
    .into_active_model();

    // get the new run id
    let run_id = active_run.clone().try_into_model()?.id;

    // create a single iteration
    let start = Utc::now().timestamp_millis();
    let iteration = iteration::ActiveModel {
        id: ActiveValue::NotSet,
        run_id: ActiveValue::Set(run_id.clone()),
        scenario_name: ActiveValue::Set("live".to_string()),
        count: ActiveValue::Set(1),
        start_time: ActiveValue::Set(start),
        stop_time: ActiveValue::Set(None),
    };
    iteration.save(db).await?;

    // start the metrics logger
    println!("wat!!");
    let mut stop_handle =
        metrics_logger::start_logging(processes_to_observe.clone(), run_id.clone(), db.clone())?;

    // keep alive!
    while let Some(_) = stop_handle.join_set.join_next().await {}

    Ok(())

    // // keep saving!
    // let shared_metrics_log = stop_handle.shared_metrics_log.clone();
    // loop {
    //     tokio::time::sleep(Duration::from_secs(1)).await;

    //     let shared_metrics_log = shared_metrics_log.clone();
    //     let mut metrics_log = shared_metrics_log.lock().unwrap();

    //     metrics_log.save(&run_id, &db).await?;
    //     metrics_log.clear();

    //     // update the iteration stop time
    //     let now = Utc::now().timestamp_millis();
    //     let mut active_iteration = dao::iteration::fetch_live(&run_id, &db)
    //         .await?
    //         .into_active_model();
    //     active_iteration.stop_time = ActiveValue::Set(now);
    //     active_iteration.update(db).await?;

    //     // update the run stop time
    //     let now = Utc::now().timestamp_millis();
    //     // let mut active_run = dao::run::fetch(run_id, &db).await?.into_active_model();
    //     active_run.stop_time = ActiveValue::Set(now);
    //     active_run.clone().update(db).await?;
    // }
}
