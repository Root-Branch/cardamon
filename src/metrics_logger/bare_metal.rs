/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::metrics::{CpuMetrics, MetricsLog};
use std::sync::{Arc, Mutex};
use sysinfo::{Pid, System};
use tokio::time::Duration;

pub enum BareMetalProcess {
    ProcId(u32),
    ProcName(String),
}

/// Enters an infinite loop logging metrics for each process to the metrics log. This function is
/// intended to be called from `metrics_logger::log_scenario` or `metrics_logger::log_live`
///
/// **WARNING**
///
/// This function should only be called from within a task that can execute it on another thread
/// otherwise it will block the main thread completely.
///
/// # Arguments
///
/// * `processes` - The processes to observe in the live environment
/// * `metrics_log` - A log of all observed metrics. Another thread should periodically save and
/// flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that it's thread is cancelled.
pub async fn keep_logging(processes: Vec<BareMetalProcess>, metrics_log: Arc<Mutex<MetricsLog>>) {
    let mut system = System::new_all();

    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        for proc in processes.iter() {
            match proc {
                BareMetalProcess::ProcId(pid) => {
                    let metrics = process_id::get_metrics(&mut system, *pid).await;
                    update_metrics_log(metrics, &metrics_log);
                }
                BareMetalProcess::ProcName(name) => {
                    let metrics = process_name::get_metrics(&mut system, name).await;
                    update_metrics_log(metrics, &metrics_log);
                }
            }
        }
    }
}

fn update_metrics_log(metrics: anyhow::Result<CpuMetrics>, metrics_log: &Arc<Mutex<MetricsLog>>) {
    match metrics {
        Ok(metrics) => metrics_log
            .lock()
            .expect("Should be able to acquire lock on metrics log")
            .push_metrics(metrics),
        Err(error) => metrics_log
            .lock()
            .expect("Should be able to acquire lock on metrics err")
            .push_error(error),
    }
}

mod process_id {
    use super::*;

    pub async fn get_metrics(system: &mut System, pid: u32) -> anyhow::Result<CpuMetrics> {
        // refresh system information
        system.refresh_all();

        if let Some(process) = system.process(Pid::from_u32(pid)) {
            let cpu_usage = process.cpu_usage() as f64;
            let core_count = system.physical_core_count().unwrap_or(0) as i32;

            let metrics = CpuMetrics {
                process_id: format!("{pid}"),
                process_name: process.name().to_string(),
                cpu_usage,
                core_count,
            };

            Ok(metrics)
        } else {
            Err(anyhow::anyhow!(format!("process with id {pid} not found")))
        }
    }
}

mod process_name {
    use super::*;

    pub async fn get_metrics(system: &mut System, name: &str) -> anyhow::Result<CpuMetrics> {
        // Refresh system information
        system.refresh_all();

        // assumes the first process with the given name
        let process = system.processes_by_name(name).next();
        match process {
            Some(process) => {
                let cpu_usage = process.cpu_usage() as f64;
                let core_count = system.physical_core_count().unwrap_or(0) as i32;

                let metrics = CpuMetrics {
                    process_id: format!("{}", process.pid()),
                    process_name: name.to_string(),
                    cpu_usage,
                    core_count,
                };

                Ok(metrics)
            }

            None => Err(anyhow::anyhow!("process with name {name} not found")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use subprocess::Exec;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    #[cfg(target_family = "windows")]
    async fn metrics_can_be_gatered_using_process_id() -> anyhow::Result<()> {
        // spawn a test process
        let mut proc = Exec::cmd("powershell")
            .arg("-Command")
            .arg(r#"while($true) {get-random | out-null}"#)
            .detached()
            .popen()
            .context("Failed to spawn detached process")?;
        let pid = proc.pid().context("Process should have a pid")?;

        // create a new sysinfo system
        let mut system = System::new_all();

        // gather metrics for a little while
        let mut metrics_log = vec![];
        let iterations = 50;
        for _ in 0..iterations {
            let metrics = process_id::get_metrics(&mut system, pid).await?;
            metrics_log.push(metrics);
            sleep(Duration::from_millis(200)).await;
        }
        proc.kill().context("Failed to kill process")?;

        // metrics log should have 10 entries
        assert_eq!(metrics_log.len(), iterations);

        // metrics should contain non-zero cpu_usage
        let cpu_usage = metrics_log.iter().fold(0_f64, |acc, metrics| {
            acc + metrics.cpu_usage / metrics.core_count as f64
        }) / iterations as f64;
        println!("{cpu_usage}");
        assert!(cpu_usage > 0_f64);

        Ok(())
    }

    #[tokio::test]
    #[cfg(target_family = "windows")]
    async fn metrics_can_be_gathered_using_process_name() -> anyhow::Result<()> {
        // spawn a test process
        let mut proc = Exec::cmd("powershell")
            .arg("-Command")
            .arg(r#"while($true) {get-random | out-null}"#)
            .detached()
            .popen()
            .context("Failed to spawn detached process")?;

        // create a new sysinfo system
        let mut system = System::new_all();

        // gather metrics for a little while
        let mut metrics_log = vec![];
        let iterations = 50;
        for _ in 0..iterations {
            let metrics = process_name::get_metrics(&mut system, "powershell").await?;
            metrics_log.push(metrics);
            sleep(Duration::from_millis(200)).await;
        }
        proc.kill().context("Failed to kill process")?;

        // metrics log should have 10 entries
        assert_eq!(metrics_log.len(), iterations);

        // metrics should contain non-zero cpu_usage
        let cpu_usage = metrics_log.iter().fold(0_f64, |acc, metrics| {
            acc + metrics.cpu_usage / metrics.core_count as f64
        }) / iterations as f64;
        println!("{cpu_usage}");
        assert!(cpu_usage > 0_f64);

        Ok(())
    }

    #[tokio::test]
    #[cfg(target_family = "windows")]
    async fn should_return_err_if_wrong_pid() {
        // create a new sysinfo System
        let mut system = System::new_all();

        // find a process id that doesn't exist
        system.refresh_all();

        let mut rand_pid = 1337;
        loop {
            if !system.processes().contains_key(&Pid::from_u32(rand_pid)) {
                break;
            } else {
                rand_pid += 1;
            }
        }

        // attempt to gather metrics
        let res = process_id::get_metrics(&mut system, rand_pid).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[cfg(target_family = "unix")]
    async fn metrics_can_be_gatered_using_process_id() -> anyhow::Result<()> {
        todo!();
    }

    #[tokio::test]
    #[cfg(target_family = "unix")]
    async fn metrics_can_be_gathered_using_process_name() -> anyhow::Result<()> {
        todo!();
    }
}
