/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::{
    metrics_server::{self, dto},
    settings::{MainConfig, Scenario},
    telegraf,
};
use spinners::{Spinner, Spinners};
use tokio::process::Command;
use tracing::{error, info};

use diesel::SqliteConnection;
use nanoid::nanoid;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
pub async fn start_scenarios(
    settings: &MainConfig,
    db_conn: Arc<Mutex<SqliteConnection>>,
) -> anyhow::Result<()> {
    let telegraf_conf_path = "telegraf.conf".into();
    let cardamon_run_id = init_scenario_run(db_conn.clone(), telegraf_conf_path).await?;

    // For each command in config, run command
    let mut scenarios_run: Vec<String> = vec![];
    for scenario in &settings.scenarios {
        // Pass in iteration number
        match run(&scenario, &cardamon_run_id).await {
            Ok(()) => scenarios_run.push(scenario.name.to_string()),
            Err(e) => error!("Error with scenario: {}", e),
        }
    }
    let summary = generate_scenario_summary(scenarios_run)?;
    info!("{}", summary);

    Ok(())
}

fn generate_scenario_summary(scenarios: Vec<String>) -> anyhow::Result<String> {
    // generate carbon summary
    let summary_opts = dto::ScenarioSummaryOpts {
        scenarios,
        last_n: 3,
        cpu_tdp: 23.0,
    };

    // request summary of scenario run compared to previous runs
    let stats = ureq::get("http://localhost:2050/scenario_summary")
        .send_json(ureq::json!(summary_opts))
        .map(|res| res.into_json::<Vec<dto::ScenarioRunStats>>())?;

    stats
        .map(|stats| {
            let mut summary = String::new();
            for stats in stats {
                summary.push_str(&format!("\n{}", stats.scenario_name));
                for stats in stats.run_stats {
                    summary.push_str(&format!(
                        "\n\t + [{}]",
                        stats.start_time.format("%Y-%m-%d @ %H:%M")
                    ));
                    for stats in stats.process_stats {
                        summary.push_str(&format!(
                            "\n\t\t {:}: \t{:.2}",
                            stats.process_name, stats.energy_consumption_w,
                        ));
                    }
                }
            }

            summary
        })
        .map_err(|err| anyhow::anyhow!(format!("{}", err.to_string())))
}

pub async fn init_scenario_run(
    state: Arc<Mutex<SqliteConnection>>,
    telegraf_conf_path: PathBuf,
) -> anyhow::Result<String> {
    // create a unique cardamon label for this scenario
    let cardamon_run_id = nanoid!();
    let cardamon_run_type = String::from("SCENARIO");

    // start the metric server
    metrics_server::start(state);

    // start telegraf
    telegraf::start(
        telegraf_conf_path,
        cardamon_run_type,
        cardamon_run_id.clone(),
        String::from("http://127.0.0.1:2050"),
    );

    // wait a second for telegraf to start
    sleep(Duration::from_millis(1000)).await;

    Ok(cardamon_run_id)
}

pub async fn run(scenario: &Scenario, cardamon_run_id: &str) -> anyhow::Result<()> {
    let scenario_name = &scenario.name;

    let mut spinner = Spinner::new(Spinners::Dots, format!("Running {}", scenario_name));

    // start measurement
    let start_time = chrono::Utc::now().timestamp_millis();

    // Split the scenario_command into a vector
    let command_parts: Vec<&str> = scenario.command.split_whitespace().collect();

    // Get the command and arguments
    let command = command_parts
        .first()
        .ok_or_else(|| anyhow::anyhow!("Empty command"))?;
    let args = &command_parts[1..];

    // run scenario ...
    let output = Command::new(command)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await?;

    // stop measurement
    let stop_time = chrono::Utc::now().timestamp_millis();

    let scenario = dto::Scenario {
        cardamon_run_type: String::from("SCENARIO"),
        cardamon_run_id: String::from(cardamon_run_id),
        scenario_name: scenario_name.clone(),
        iteration: scenario.iteration,
        start_time,
        stop_time,
    };

    // send scenario run to db
    ureq::post("http://localhost:2050/scenario")
        .send_json(ureq::json!(scenario))
        .map_err(anyhow::Error::msg)
        .map(|_| ())?;

    spinner.stop_with_symbol("✓");

    if output.status.success() {
        Ok(())
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}",
            error_message
        ))
    }
}

pub fn print_scenario_stats(stats: &Vec<dto::ScenarioRunStats>) {
    for stats in stats {
        println!("\n+ {}", stats.scenario_name);
        for stats in &stats.run_stats {
            println!("\t + [{}]", stats.start_time.format("%Y-%m-%d @ %H:%M"));
            for stats in &stats.process_stats {
                println!(
                    "\t\t {:}: \t{:.2} W",
                    stats.process_name, stats.energy_consumption_w,
                );
            }
        }
    }
}
