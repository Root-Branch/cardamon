/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::path::PathBuf;
use tokio::{process::Command, task::JoinHandle};
use tracing::{error, info};

pub fn start(
    conf_path: PathBuf,
    cardamon_run_type: String,
    cardamon_run_id: String,
    metric_server_url: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut child = Command::new("telegraf")
            .envs(vec![
                ("CARDAMON_RUN_TYPE", cardamon_run_type),
                ("CARDAMON_RUN_ID", cardamon_run_id),
                ("METRIC_SERVER_URL", metric_server_url),
            ])
            .arg("--config")
            .arg(conf_path)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to start Telegraf");

        match child.wait().await {
            Ok(status) => {
                if status.success() {
                    info!("Telegraf exited successfully");
                } else {
                    error!("Telegraf exited with an error");
                }
            }
            Err(e) => {
                error!("Error waiting for Telegraf: {}", e);
            }
        }
    })
}
