use crate::{
    execution_plan::ProcessToObserve,
    metrics::{CpuMetrics, MetricsLog},
};
use chrono::Utc;
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};
use sysinfo::{Pid, System};
use tokio::time::Duration;
use tracing::trace;

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
/// * `pids` - The process ids to observe
/// * `metrics_log` - A log of all observed metrics. Another thread should periodically save and
///                   flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that it's thread is cancelled.
pub async fn keep_logging(
    processes_to_observe: Vec<ProcessToObserve>,
    metrics_log: Arc<Mutex<MetricsLog>>,
) -> anyhow::Result<()> {
    let mut system = System::new_all();

    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        system.refresh_all();
        for process_to_observe in processes_to_observe.iter() {
            match process_to_observe {
                ProcessToObserve::ExternalPid(pid) => {
                    let metrics = get_metrics(&mut system, *pid).await?;
                    update_metrics_log(metrics, &metrics_log);
                }

                ProcessToObserve::ManagedPid {
                    process_name,
                    pid,
                    down: _,
                } => {
                    let mut metrics = get_metrics(&mut system, *pid).await?;
                    metrics.process_name = process_name.clone();
                    update_metrics_log(metrics, &metrics_log);
                }

                _ => panic!(),
            }
        }
    }
}

fn update_metrics_log(metrics: CpuMetrics, metrics_log: &Arc<Mutex<MetricsLog>>) {
    metrics_log
        .lock()
        .expect("Should be able to acquire lock on metrics log")
        .push_metrics(metrics);
}

async fn get_metrics(system: &mut System, pid: u32) -> anyhow::Result<CpuMetrics> {
    if let Some(process) = system.process(Pid::from_u32(pid)) {
        let core_count = num_cpus::get_physical() as i32;

        // Cores can be 0, or system can be wrong, therefore divide here
        let cpu_usage = process.cpu_usage() as f64 / 100.0;
        let timestamp = Utc::now().timestamp_millis();
        // Updated, .name just gives "bash" etc, short version
        // .exe gives proper path
        let process_name: String = process
            .exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                let process_name = process.name().to_os_string();
                let name_str = process_name.to_string_lossy();
                name_str.deref().to_string()
            });

        trace!("[PID {}] cpu_usage: {:?}", process.pid(), cpu_usage);
        let metrics = CpuMetrics {
            process_id: format!("{pid}"),
            process_name,
            cpu_usage,
            core_count,
            timestamp,
        };

        Ok(metrics)
    } else {
        Err(anyhow::anyhow!(format!("process with id {pid} not found")))
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
            let metrics = get_metrics(&mut system, pid).await?;
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
        let res = get_metrics(&mut system, rand_pid).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[cfg(target_family = "unix")]
    async fn metrics_can_be_gatered_using_process_id() -> anyhow::Result<()> {
        // spawn a test process

        use subprocess::NullFile;
        let mut proc = Exec::cmd("bash")
            .arg("-c")
            .arg("while true; do shuf -i 0-1337 -n 1; done")
            .detached()
            .stdout(NullFile)
            .popen()
            .context("Failed to spawn detached process")?;
        let pid = proc.pid().context("Process should have a pid")?;

        // create a new sysinfo system
        let mut system = System::new_all();
        system.refresh_all();

        // gather metrics for a little while
        let mut metrics_log = vec![];
        let iterations = 50;
        for _ in 0..iterations {
            let metrics = get_metrics(&mut system, pid).await?;
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
        assert!(cpu_usage > 0_f64);

        Ok(())
    }
}
