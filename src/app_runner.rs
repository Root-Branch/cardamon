/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::config::{run::ProcessToObserve, Process};
use anyhow::{anyhow, Context};
use subprocess::Exec;

/// Runs the given command as a detached processes. This function does not block because the
/// process is managed by the OS and running separately from this thread.
///
/// # Arguments
///
/// * command - The command to run.
///
/// # Returns
///
/// The PID returned by the operating system
fn run_command(command: &str) -> anyhow::Result<u32> {
    // split command string into command and args
    match &command.split(' ').collect::<Vec<_>>()[..] {
        [command, args @ ..] => {
            let mut exec = Exec::cmd(command);
            for arg in args {
                exec = exec.arg(arg);
            }

            exec.detached()
                .popen()
                .context("Failed to spawn detached process")?
                .pid()
                .context("Process should have a PID")
        }
        _ => Err(anyhow!("")),
    }
}

/// Run the given process as a detached process and return a list of all things to observe (in
/// Docker it's possible to have a single docker compose process which starts multiple containers).
///
/// # Arguments
///
/// * proc - The Process to run
///
/// # Returns
///
/// A list of all the processes to observe
pub fn run_process(proc: &Process) -> anyhow::Result<Vec<ProcessToObserve>> {
    match proc {
        Process::Docker {
            name: _,
            containers,
            command,
        } => {
            // run the command
            run_command(command)?;

            // return the containers as vector of ProcessToObserve
            Ok(containers
                .iter()
                .map(|name| ProcessToObserve::ContainerName(name.clone()))
                .collect())
        }

        Process::BareMetal { name: _, command } => {
            // run the command
            let pid = run_command(command)?;

            // return the pid as a ProcessToObserve
            Ok(vec![ProcessToObserve::BareMetalId(pid)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::panic;

    #[test]
    fn bare_metal_process_should_run() -> anyhow::Result<()> {
        let pid = run_command("powershell sleep 5")?;

        assert!(pid > 0);

        Ok(())
    }

    #[test]
    fn convert_process_to_process_to_observe() -> anyhow::Result<()> {
        let proc = Process::BareMetal {
            name: "sleep_for_time".to_string(),
            command: "powershell sleep 5".to_string(),
        };

        match run_process(&proc)?.first().context("Expected a vec")? {
            ProcessToObserve::BareMetalId(id) => assert!(*id > 0),
            _ => panic!(""),
        }

        Ok(())
    }
}
