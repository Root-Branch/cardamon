/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::metrics_server::dto;
use spinners::{Spinner, Spinners};
use tokio::process::Command;

pub async fn run(scenario_command: &str, cardamon_run_id: &str) -> anyhow::Result<String> {
    let scenario_name = scenario_command.to_string();

    let mut spinner = Spinner::new(Spinners::Dots, format!("Running {}", scenario_name));

    // start measurement
    let start_time = chrono::Utc::now().timestamp_millis();

    // Split the scenario_command into a vector
    let command_parts: Vec<&str> = scenario_command.split_whitespace().collect();

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
        scenario_name: String::from(scenario_name.clone()),
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
        Ok(scenario_name)
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
