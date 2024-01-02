/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::metrics_server::dto;
use spinners::{Spinner, Spinners};
use std::{ffi::OsStr, path::PathBuf};
use tokio::process::Command;

pub async fn run<'a>(scenario_path: &'a PathBuf, cardamon_run_id: &str) -> anyhow::Result<&'a str> {
    let file_name = scenario_path.file_name().and_then(OsStr::to_str);
    let ext = scenario_path.extension().and_then(OsStr::to_str);

    let is_file = scenario_path.is_file();
    let has_valid_name = file_name.unwrap_or("") != "";
    let has_js_ext = ext.unwrap_or("") == "js";

    if is_file && has_valid_name && has_js_ext {
        let scenario_name = file_name.unwrap();

        let mut spinner = Spinner::new(Spinners::Dots, format!("Running {}", scenario_name));

        // start measurement
        let start_time = chrono::Utc::now().timestamp_millis();

        // run scenario ...
        Command::new("node")
            .arg(scenario_path.clone())
            .kill_on_drop(true)
            .output()
            .await?;

        // stop measurement
        let stop_time = chrono::Utc::now().timestamp_millis();

        let scenario = dto::Scenario {
            cardamon_run_type: String::from("SCENARIO"),
            cardamon_run_id: String::from(cardamon_run_id),
            scenario_name: String::from(scenario_name),
            start_time,
            stop_time,
        };

        // send scenario run to db
        ureq::post("http://localhost:2050/scenario")
            .send_json(ureq::json!(scenario))
            .map_err(anyhow::Error::msg)
            .map(|_| ())?;

        spinner.stop_with_symbol("✓");

        Ok(scenario_name)
    } else {
        Err(anyhow::anyhow!(
            "{:?} is not a valid javascript file",
            scenario_path
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
