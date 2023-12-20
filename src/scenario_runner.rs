/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::metrics_server::dto;
use anyhow::Result;
use log::debug;
use std::{fs, path::PathBuf};
use tokio::process::Command;

pub async fn run(
    scenarios_path: &PathBuf,
    cardamon_run_type: &str,
    cardamon_run_id: &str,
) -> Result<Vec<String>> {
    let dir_entries = fs::read_dir(scenarios_path)?;

    let mut scenarios_run: Vec<String> = vec![];

    for dir_entry in dir_entries {
        let scenario_path = dir_entry?.path();

        if scenario_path.is_file() || scenario_path.is_symlink() {
            match (scenario_path.file_name(), scenario_path.extension()) {
                (Some(filename), Some(ext)) if ext == "js" => {
                    if let Ok(scenario_name) = filename.to_os_string().into_string() {
                        debug!("running scenario {:?}", scenario_name);

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
                            cardamon_run_type: String::from(cardamon_run_type),
                            cardamon_run_id: String::from(cardamon_run_id),
                            scenario_name: scenario_name.clone(),
                            start_time,
                            stop_time,
                        };

                        // send scenario run to db
                        ureq::post("http://localhost:8000/scenario")
                            .send_json(ureq::json!(scenario))?;

                        scenarios_run.push(scenario_name);
                    }
                }

                _ => {}
            }
        }
    }

    Ok(scenarios_run)
}
