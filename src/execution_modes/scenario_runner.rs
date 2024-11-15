use crate::{
    config::Scenario,
    data::{dataset::LiveDataFilter, dataset_builder::DatasetBuilder, Data},
    entities::{iteration, run},
    execution_plan::ProcessToObserve,
    metrics_logger,
    models::rab_model,
    process_control::shutdown_processes,
};
use anyhow::{anyhow, Context};
use chrono::Utc;
use colored::*;
use itertools::*;
use nanoid::nanoid;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, IntoActiveModel};
use term_table::{row, row::Row, rows, table_cell::*, Table, TableStyle};
use tracing::info;

pub async fn run_scenario<'a>(
    run_id: &str,
    scenario: &Scenario,
    iteration: i32,
) -> anyhow::Result<iteration::ActiveModel> {
    let start = Utc::now().timestamp_millis();

    // Split the scenario_command into a vector
    let command_parts = match shlex::split(&scenario.command) {
        Some(command) => command,
        None => vec!["error".to_string()],
    };

    // Get the command and arguments
    let command = command_parts
        .first()
        .ok_or_else(|| anyhow::anyhow!("Empty command"))?;
    let args = &command_parts[1..];

    // run scenario ...
    let output = tokio::process::Command::new(command)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .context(format!("Tokio command failed to run {command}"))?;
    info!("Ran command {}", scenario.command);
    if output.status.success() {
        let stop = Utc::now().timestamp_millis();

        let scenario_iteration = iteration::ActiveModel {
            id: ActiveValue::NotSet,
            run_id: ActiveValue::Set(run_id.to_string()),
            scenario_name: ActiveValue::Set(scenario.name.clone()),
            count: ActiveValue::Set(iteration),
            start_time: ActiveValue::Set(start),
            stop_time: ActiveValue::Set(Some(stop)),
        };
        Ok(scenario_iteration)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!(
            "Scenario execution failed: {}. Command: {}",
            error_message,
            scenario.command
        ))
    }
}

pub async fn run_scenarios<'a>(
    cpu_id: i32,
    region: &Option<String>,
    ci: f64,
    scenarios: Vec<&'a Scenario>,
    processes_to_observe: Vec<ProcessToObserve>,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    let start_time = Utc::now().timestamp_millis();

    // create a new run
    let run_id = nanoid!(5, &nanoid::alphabet::SAFE);
    let mut active_run = run::ActiveModel {
        id: ActiveValue::Set(run_id.clone()),
        is_live: ActiveValue::Set(false),
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
    // let run_id = active_run.clone().try_into_model()?.id;
    println!("{}", &run_id);

    // ---- for each scenario ----
    for scenario in scenarios {
        // for each iteration
        for iteration in 1..scenario.iterations + 1 {
            println!(
                "> running scenario {} - iteration {}/{}",
                scenario.name.green(),
                iteration,
                scenario.iterations
            );

            // start the metrics loggers
            let stop_handle = metrics_logger::start_logging(
                processes_to_observe.clone(),
                run_id.clone(),
                db.clone(),
            )?;

            // run the scenario
            let scenario_iteration = run_scenario(&run_id, &scenario, iteration).await?;
            scenario_iteration.save(db).await?;

            // stop the metrics loggers
            stop_handle.stop().await;
        }
    }

    let stop_time = Utc::now().timestamp_millis(); // Use UTC to avoid confusion, UI can handle
                                                   // timezones

    // update run with the stop time
    active_run.stop_time = ActiveValue::Set(Some(stop_time));
    active_run.save(db).await?;

    // stop the application
    shutdown_processes(&processes_to_observe)?;

    // create a dataset containing the data just collected
    let observation_dataset_rows = DatasetBuilder::new().scenarios_in_run(&run_id).all();
    let observation_dataset = observation_dataset_rows
        .last_n_runs(5)
        .all()
        .build(&db)
        .await?;

    println!("\n{}", " Summary ".reversed().green());
    for scenario_dataset in observation_dataset
        .by_scenario(LiveDataFilter::ExcludeLive)
        .iter()
    {
        let run_datasets = scenario_dataset.by_run();

        // execute model for current run
        let (head, tail) = run_datasets
            .split_first()
            .expect("Dataset does not include recent run.");
        let run_data = head.apply_model(&db, &rab_model).await?;

        // execute model for previous runs and calculate trend
        let mut tail_data = vec![];
        for run_dataset in tail {
            let run_data = run_dataset.apply_model(&db, &rab_model).await?;
            tail_data.push(run_data.data);
        }
        let tail_data = Data::mean(&tail_data.iter().collect_vec());
        let trend = run_data.data.pow - tail_data.pow;
        let trend_str = match trend.is_nan() {
            true => "--".bright_black(),
            false => {
                if trend > 0.0 {
                    format!("↑ {:.3}Wh", trend.abs()).red()
                } else {
                    format!("↓ {:.3}Wh", trend).green()
                }
            }
        };

        println!("{}:", scenario_dataset.scenario_name().to_string().green());

        let table = Table::builder()
            .rows(rows![
                row![
                    TableCell::builder("Region").build(),
                    TableCell::builder("Duration (s)".bold()).build(),
                    TableCell::builder("Power (Wh)".bold()).build(),
                    TableCell::builder("CI (gWh)".bold()).build(),
                    TableCell::builder("CO2 (g)".bold()).build(),
                    TableCell::builder(format!("Trend (over {} runs)", tail.len()).bold()).build()
                ],
                row![
                    TableCell::new(format!("{}", run_data.region.clone().unwrap_or_default())),
                    TableCell::new(
                        run_data
                            .duration()
                            .map(|dur| format!("{:.3}s", dur))
                            .unwrap_or("--".to_string())
                    ),
                    TableCell::new(format!("{:.3}Wh", run_data.data.pow)),
                    TableCell::new(format!("{:.3}gWh", run_data.ci)),
                    TableCell::new(format!("{:.3}g", run_data.data.co2)),
                    TableCell::new(trend_str)
                ]
            ])
            .style(TableStyle::rounded())
            .build();

        println!("{}", table.render())
    }

    Ok(())
}
