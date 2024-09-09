pub mod config;
pub mod dao;
pub mod data;
pub mod entities;
pub mod metrics;
pub mod metrics_logger;
pub mod migrations;
pub mod models;
pub mod server;

use crate::{
    config::Config,
    data::{dataset::Dataset, dataset_builder::DatasetBuilder},
    migrations::{Migrator, MigratorTrait},
};
use anyhow::{anyhow, Context};
use chrono::Utc;
use colored::Colorize;
use config::{ExecutionPlan, ProcessToObserve, ProcessType, Redirect, ScenarioToExecute};
use entities::{iteration, run};
use sea_orm::*;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::mpsc::channel,
};
use subprocess::{Exec, NullFile, Redirection};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tracing::{debug, info};

fn ask_for_cpu() -> String {
    loop {
        print!("Please enter a CPU name: ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let res = std::io::stdin().read_line(&mut input);
        match res {
            Ok(_) => return input,
            Err(_) => continue,
        }
    }
}

fn ask_for_tdp() -> f64 {
    loop {
        print!("Please enter the TDP of your CPU in watts: ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let res = std::io::stdin().read_line(&mut input);
        match res {
            Ok(_) => match input.trim().parse::<f64>() {
                Ok(parsed_input) => {
                    return parsed_input;
                }
                Err(_) => {
                    println!("{}", "Please enter a valid number.".yellow());
                    continue;
                }
            },
            Err(_) => continue,
        }
    }
}

fn find_cpu() -> Option<String> {
    let sys = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    sys.cpus().first().map(|cpu| cpu.brand().to_string())
}

fn parse_json(json_obj: &Value) -> Option<f64> {
    json_obj
        .get("verbose")?
        .get("avg_power")?
        .get("value")?
        .as_f64()
}

async fn fetch_tdp(cpu_name: &str) -> anyhow::Result<f64> {
    let client = reqwest::Client::new();
    let mut json = HashMap::new();
    json.insert("name", cpu_name);

    let resp = client
        .post("https://api.boavizta.org/v1/component/cpu")
        .header("Content-Type", "application/json")
        .json(&json)
        .send()
        .await?;

    let json_obj = resp.json().await?;
    parse_json(&json_obj).context("Error finding CPU stats from Boavizta")
}

/// Attempts to find the users CPU automatically and asks the user to enter it manually if that
/// fails.
pub async fn init_config() {
    let cpu_name: String;

    println!("\n{}", " Setting up Cardamon ".reversed().green());
    loop {
        print!("Would you like to create a config for this computer [1] or another computer [2]? ");
        let _ = std::io::stdout().flush();

        let mut ans = String::new();
        let res = std::io::stdin().read_line(&mut ans);
        match res {
            Ok(_) => {
                let opt = ans.trim().parse::<u32>();
                match opt {
                    Ok(1) => {
                        cpu_name = match find_cpu() {
                            Some(name) => {
                                println!("{} {}", "It looks like you have a".yellow(), name);
                                name
                            }
                            None => {
                                println!("{}", "Unable to find CPU!".red());
                                ask_for_cpu()
                            }
                        };
                        break;
                    }
                    Ok(2) => {
                        cpu_name = ask_for_cpu();
                        break;
                    }
                    _ => {
                        println!("{}", "Please enter 1 or 2.\n".yellow());
                        continue;
                    }
                }
            }
            Err(_) => {
                println!("{}", "Please enter 1 or 2.\n".yellow());
                continue;
            }
        }
    }

    let tdp = match fetch_tdp(&cpu_name).await {
        Ok(tdp) => {
            println!("{} {}", "Boavista reports an avg power of".yellow(), tdp);
            tdp
        }
        Err(_) => {
            println!("{}", "Cannot get avg power from Boavizta!".red());
            ask_for_tdp()
        }
    };

    match Config::write_example_to_file(&cpu_name, tdp, Path::new("./cardamon.toml")) {
        Ok(_) => {
            println!("{}", "cardamon.toml created!".green());
            println!("\n🤩\n");
        }

        Err(err) => {
            println!("{}\n{}", "Error creating config.".red(), err);
            println!("\n😭\n");
        }
    }
}

pub async fn db_connect(
    database_url: &str,
    database_name: Option<&str>,
) -> anyhow::Result<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    match db.get_database_backend() {
        DbBackend::Sqlite => Ok(db),

        DbBackend::Postgres => {
            let database_name =
                database_name.context("Database name is required for postgres connections")?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE \"{}\";", database_name),
            ))
            .await
            .ok();

            let url = format!("{}/{}", database_url, database_name);
            Database::connect(&url)
                .await
                .context("Error creating postgresql database.")
        }

        DbBackend::MySql => {
            let database_name =
                database_name.context("Database name is required for mysql connections")?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE IF NOT EXISTS `{}`;", database_name),
            ))
            .await?;

            let url = format!("{}/{}", database_url, database_name);
            Database::connect(&url)
                .await
                .context("Error creating mysql database.")
        }
    }
}

pub async fn db_migrate(db_conn: &DatabaseConnection) -> anyhow::Result<()> {
    Migrator::up(db_conn, None)
        .await
        .context("Error migrating database.")
}

/// Deletes previous runs .stdout and .stderr
/// Stdout and stderr capturing are append due to a scenario / observeration removing previous ones
/// stdout and err
pub fn cleanup_stdout_stderr() -> anyhow::Result<()> {
    debug!("Cleaning up stdout and stderr");
    let stdout = Path::new("./.stdout");
    let stderr = Path::new("./.stderr");
    if stdout.exists() {
        fs::remove_file(stdout)?;
    }
    if stderr.exists() {
        fs::remove_file(stderr)?;
    }
    Ok(())
}

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
fn run_command_detached(command: &str, redirect: &Option<Redirect>) -> anyhow::Result<u32> {
    let redirect = redirect.unwrap_or(Redirect::File);

    // break command string into POSIX words
    let words = shlex::split(command).expect("Command string is not POSIX compliant.");

    // split command string into command and args
    match &words[..] {
        [command, args @ ..] => {
            let exec = Exec::cmd(command).args(args);
            // for arg in args {
            //     exec = exec.arg(arg);
            // }
            //

            let exec = match redirect {
                Redirect::Null => exec.stdout(NullFile).stderr(NullFile),
                Redirect::Parent => exec,
                Redirect::File => {
                    let out_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open("./.stdout")?;
                    let err_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open("./.stderr")?;
                    exec.stdout(Redirection::File(out_file))
                        .stderr(Redirection::File(err_file))
                }
            };

            exec.detached()
                .popen()
                .context(format!(
                    "Failed to spawn detached process, command: {}",
                    command
                ))?
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
fn run_process(proc: &config::ProcessToExecute) -> anyhow::Result<Vec<ProcessToObserve>> {
    match &proc.process {
        config::ProcessType::Docker { containers } => {
            debug!("Running command {} in detached mode ( Docker ) ", proc.up);
            // run the command
            run_command_detached(&proc.up, &proc.redirect)?;

            // return the containers as vector of ProcessToObserve
            Ok(containers
                .iter()
                .map(|name| ProcessToObserve::ContainerName(name.clone()))
                .collect())
        }

        config::ProcessType::BareMetal => {
            debug!(
                "Running command {} in detached mode ( Bare metal ) ",
                proc.up
            );
            // run the command
            let pid = run_command_detached(&proc.up, &proc.redirect)?;

            // return the pid as a ProcessToObserve
            Ok(vec![ProcessToObserve::Pid(Some(proc.name.clone()), pid)])
        }
    }
}

async fn run_scenario<'a>(
    run_id: i32,
    scenario_to_execute: &ScenarioToExecute<'a>,
    iteration: i32,
) -> anyhow::Result<iteration::ActiveModel> {
    let start = Utc::now().timestamp_millis();

    // Split the scenario_command into a vector
    let command_parts = match shlex::split(&scenario_to_execute.scenario.command) {
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
    info!("Ran command {}", scenario_to_execute.scenario.command);
    if output.status.success() {
        let stop = Utc::now().timestamp_millis();

        let scenario_iteration = iteration::ActiveModel {
            id: ActiveValue::NotSet,
            run_id: ActiveValue::Set(run_id),
            scenario_name: ActiveValue::Set(scenario_to_execute.scenario.name.clone()),
            count: ActiveValue::Set(iteration),
            start_time: ActiveValue::Set(start),
            stop_time: ActiveValue::Set(stop),
        };
        Ok(scenario_iteration)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}. Command: {}",
            error_message,
            scenario_to_execute.scenario.command
        ))
    }
}

fn shutdown_application(
    exec_plan: &ExecutionPlan,
    running_processes: &[ProcessToObserve],
) -> anyhow::Result<()> {
    // for each process in the execution plan that has a "down" command, attempt to run that
    // command.
    for proc in exec_plan.processes_to_execute.iter() {
        if let Some(down_command) = &proc.down {
            match proc.process {
                ProcessType::BareMetal => {
                    print!("> stopping process {}", proc.name.green());

                    // find the pid associated with this process
                    let pid = running_processes.iter().find_map(|p| match p {
                        ProcessToObserve::Pid(Some(name), pid) if name == &proc.name => Some(*pid),
                        _ => None,
                    });

                    // if pid can't be found then log an error
                    if let Some(pid) = pid {
                        // replace {pid} with the actual PID in the down command
                        let down_command = down_command.replace("{pid}", &pid.to_string());

                        let res = run_command_detached(&down_command, &proc.redirect);
                        if res.is_err() {
                            let err = res.unwrap_err();
                            tracing::warn!(
                                "Failed to shutdown process with name {}\n{}",
                                proc.name,
                                err
                            );
                            println!();
                        } else {
                            println!("\t{}", "✓".green());
                            println!("\t{}", format!("- {}", down_command).bright_black());
                        }
                    } else {
                        tracing::warn!(
                            "Unable to find PID for bare-metal process with name: {}",
                            proc.name
                        );
                        println!();
                    }
                }
                ProcessType::Docker { containers: _ } => {
                    let res = run_command_detached(down_command, &proc.redirect);
                    if res.is_err() {
                        let err = res.unwrap_err();
                        tracing::warn!(
                            "Failed to shutdown process with name {}\n{}",
                            proc.name,
                            err
                        );
                        println!();
                    } else {
                        println!("\t{}", "✓".green());
                        println!("\t{}", format!("- {}", down_command).bright_black());
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn run<'a>(
    exec_plan: ExecutionPlan<'a>,
    cpu_avg_power: f32,
    db: &DatabaseConnection,
) -> anyhow::Result<Dataset> {
    let mut run_id: i32 = 0;

    let mut processes_to_observe = exec_plan.external_processes_to_observe.to_vec(); // external procs to observe are cloned here.

    // run the application if there is anything to run
    if !exec_plan.processes_to_execute.is_empty() {
        for proc in exec_plan.processes_to_execute.iter() {
            print!("> starting process {}", proc.name.green());
            let process_to_observe = run_process(proc)?;
            processes_to_observe.extend(process_to_observe);
            println!("{}", "\t✓".green());
            println!("\t{}", format!("- {}", proc.up).bright_black());
        }
    }

    // set ctrlc handler
    let (tx, rx) = channel();
    ctrlc::set_handler(move || {
        tx.send(())
            .expect("Could not send ctrl-c signal for graceful shutdown")
    })
    .expect("");

    // ---- for each scenario ----
    for scenario_to_execute in exec_plan.scenarios_to_execute.iter() {
        let start_time = Utc::now().timestamp_millis(); // Use UTC to avoid confusion, UI can handle
                                                        // timezones

        // create a new run
        let mut active_run = run::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            cpu_avg_power: sea_orm::ActiveValue::Set(cpu_avg_power),
            start_time: sea_orm::ActiveValue::Set(start_time),
            stop_time: sea_orm::ActiveValue::set(None),
        }
        .save(db)
        .await?;

        // get the new run id
        run_id = active_run.clone().try_into_model()?.id;

        // for each iteration
        for iteration in 1..scenario_to_execute.scenario.iterations + 1 {
            println!(
                "> running scenario {} - iteration {}/{}",
                scenario_to_execute.scenario.name.green(),
                iteration,
                scenario_to_execute.scenario.iterations
            );

            // start the metrics loggers
            let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

            // run the scenario
            let scenario_iteration = run_scenario(run_id, scenario_to_execute, iteration).await?;

            // stop the metrics loggers
            let metrics_log = stop_handle.stop().await?;

            // if metrics log contains errors then display them to the user and don't save anything
            if metrics_log.has_errors() {
                // log all the errors
                for err in metrics_log.get_errors() {
                    tracing::error!("{}", err);
                }
                return Err(anyhow!("Metric log contained errors, please see logs."));
            }
            //
            // save the scenario iteration
            scenario_iteration.save(db).await?;

            for metrics in metrics_log.get_metrics() {
                metrics.into_active_model(run_id).save(db).await?;
            }
        }

        // write scenario and metrics to db
        // write run table first due to foreign key constraints
        //let stop_time = time::SystemTime::now().duration_since(time::UNIX_EPOCH).
        let stop_time = Utc::now().timestamp_millis(); // Use UTC to avoid confusion, UI can handle
                                                       // timezones

        // update run with the stop time
        active_run.stop_time = ActiveValue::Set(Some(stop_time));
        active_run.save(db).await?;

        match rx.try_recv() {
            Ok(_) => {
                shutdown_application(&exec_plan, &processes_to_observe).expect(
                    "Error gracefully shutting down application. Please cleanup processes manually.",
                );
                break;
            }
            Err(_) => continue,
        }
    }

    // stop the application
    shutdown_application(&exec_plan, &processes_to_observe)?;

    // create a dataset containing the data just collected
    DatasetBuilder::new(db)
        .scenarios_in_run(run_id)
        .all()
        .last_n_runs(3)
        .await
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        config::{ProcessToExecute, ProcessType},
        fetch_tdp, metrics_logger, run_process, ProcessToObserve,
    };
    use std::time::Duration;
    use sysinfo::{Pid, System};

    pub async fn setup_fixtures(fixtures: &[&str], db: &DatabaseConnection) -> anyhow::Result<()> {
        for path in fixtures {
            let path = Path::new(path);
            let stmt = std::fs::read_to_string(path)?;
            db.query_one(Statement::from_string(DatabaseBackend::Sqlite, stmt))
                .await
                .context(format!("Error applying fixture {:?}", path))?;
        }

        Ok(())
    }

    #[test]
    fn should_find_cpu() {
        let cpu_name = find_cpu();
        assert!(cpu_name.is_some())
    }

    #[tokio::test]
    async fn fetch_tdp_should_work() -> anyhow::Result<()> {
        let cpu_name = find_cpu();

        if let Some(cpu_name) = cpu_name {
            let tdp = fetch_tdp(&cpu_name).await?;
            assert!(tdp > 0f64);
            return Ok(());
        }

        panic!()
    }

    #[cfg(target_family = "windows")]
    mod windows {
        use super::*;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "powershell sleep 15".to_string(),
                down: None,
                redirect: None,
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;

            assert_eq!(processes_to_observe.len(), 1);

            match processes_to_observe.first().expect("process should exist") {
                ProcessToObserve::Pid(_, pid) => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(*pid));
                    assert!(proc.is_some());
                }

                _ => panic!("expected to find a process id"),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "powershell sleep 20".to_string(),
                down: None,
                redirect: None,
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;
            let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

            tokio::time::sleep(Duration::from_secs(10)).await;

            let metrics_log = stop_handle.stop().await?;

            assert!(!metrics_log.has_errors());
            assert!(!metrics_log.get_metrics().is_empty());

            Ok(())
        }
    }

    #[cfg(target_family = "unix")]
    mod unix {
        use std::ops::Deref;

        use super::*;
        use crate::config::Redirect;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "sleep 15".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;

            assert_eq!(processes_to_observe.len(), 1);

            match processes_to_observe.first().expect("process should exist") {
                ProcessToObserve::Pid(name, pid) => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(*pid));
                    let proc_name = proc.unwrap().name().to_os_string();
                    let proc_name = proc_name.to_string_lossy();
                    let proc_name = proc_name.deref().to_string();
                    assert!(proc.is_some());
                    assert!(proc_name == name.clone().unwrap());
                }

                e => panic!("expected to find a process id {:?}", e),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "sleep 20".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;
            let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

            tokio::time::sleep(Duration::from_secs(10)).await;

            let metrics_log = stop_handle.stop().await?;

            assert!(!metrics_log.has_errors());
            assert!(!metrics_log.get_metrics().is_empty());

            Ok(())
        }
    }
}
