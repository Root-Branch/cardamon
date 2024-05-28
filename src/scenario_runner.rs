/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use crate::settings::Scenario;
use tokio::process::Command;

pub async fn run_scenario(scenario: &Scenario) -> anyhow::Result<String> {
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
        .kill_on_drop(true) // TODO: Remove? I'm not sure this is required because we are awaiting the command
        .output()
        .await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}",
            error_message
        ))
    }
}
