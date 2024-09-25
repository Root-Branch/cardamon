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
    config::{Config, ExecutionMode},
    data::dataset_builder::DatasetBuilder,
    migrations::{Migrator, MigratorTrait},
};
use anyhow::{anyhow, Context};
use chrono::Utc;
use colored::Colorize;
use config::{ExecutionPlan, Power, Process, ProcessToObserve, ProcessType, Redirect, Scenario};
use data::dataset_builder::DatasetRows;
use entities::{cpu, iteration, run};
use sea_orm::*;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::exit,
    time::Duration,
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

fn ask_for_tdp() -> Power {
    loop {
        print!("Please enter the TDP of your CPU in watts: ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let res = std::io::stdin().read_line(&mut input);
        match res {
            Ok(_) => match input.trim().parse::<f64>() {
                Ok(parsed_input) => {
                    return Power::Tdp(parsed_input);
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

fn try_power_curve(json_obj: &Value) -> Option<Power> {
    let params_obj = json_obj.get("verbose")?.get("params")?.get("value")?;

    let a = params_obj.get("a")?.as_f64()?;
    let b = params_obj.get("b")?.as_f64()?;
    let c = params_obj.get("c")?.as_f64()?;
    let d = params_obj.get("d")?.as_f64()?;

    Some(Power::Curve(a, b, c, d))
}

fn try_tdp(json_obj: &Value) -> Option<Power> {
    let tdp = json_obj
        .get("verbose")?
        .get("tdp")?
        .get("value")?
        .as_f64()?;

    Some(Power::Tdp(tdp))
}

async fn fetch_power(cpu_name: &str) -> anyhow::Result<Power> {
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

    try_power_curve(&json_obj)
        .or(try_tdp(&json_obj))
        .context("Error fetching power from Boavizta!")
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

    let power = match fetch_power(&cpu_name).await {
        Ok(pow @ Power::Curve(a, b, c, d)) => {
            let peak_pow = a * (b * (100.0 + c)).ln() + d;
            println!(
                "{} {}",
                "Boavista reports a peak power of".yellow(),
                peak_pow
            );
            pow
        }

        Ok(pow @ Power::Tdp(tdp)) => {
            println!("{} {}", "Boavizta reports a tdp of".yellow(), tdp);
            pow
        }

        Err(_) => {
            println!("{}", "Cannot get avg power from Boavizta!".red());
            ask_for_tdp()
        }
    };

    match Config::write_example_to_file(&cpu_name, power, Path::new("./cardamon.toml")) {
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

fn shutdown_application(running_processes: &Vec<ProcessToObserve>) -> anyhow::Result<()> {
    // for each process in the execution plan that has a "down" command, attempt to run that
    // command.
    for proc in running_processes {
        match proc {
            ProcessToObserve::ManagedPid {
                pid: _,
                process_name,
                down: Some(down),
            } => {
                print!("> stopping process {}", process_name.green());

                let res = run_command_detached(&down, None);
                if res.is_err() {
                    let err = res.unwrap_err();
                    tracing::warn!(
                        "Failed to shutdown process with name {}\n{}",
                        process_name,
                        err
                    );
                    println!();
                } else {
                    println!("\t{}", "✓".green());
                    println!("\t{}", format!("- {}", down).bright_black());
                }
            }

            ProcessToObserve::ManagedContainers {
                process_name,
                container_names: _,
                down: Some(down),
            } => {
                print!("> stopping process {}", process_name.green());

                let res = run_command_detached(&down, None);
                if res.is_err() {
                    let err = res.unwrap_err();
                    tracing::warn!(
                        "Failed to shutdown process with name {}\n{}",
                        process_name,
                        err
                    );
                    println!();
                } else {
                    println!("\t{}", "✓".green());
                    println!("\t{}", format!("- {}", down).bright_black());
                }
            }

            _ => {} // do nothing!
        }
    }

    Ok(())
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
fn run_command_detached(command: &str, redirect: Option<Redirect>) -> anyhow::Result<u32> {
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
fn run_process(proc_to_exec: &Process) -> anyhow::Result<ProcessToObserve> {
    match &proc_to_exec.process_type {
        ProcessType::Docker { containers } => {
            debug!(
                "Running command {} in detached mode ( Docker ) ",
                proc_to_exec.up
            );
            // run the command
            run_command_detached(&proc_to_exec.up, proc_to_exec.redirect)?;

            // return the containers as vector of ProcessToObserve
            Ok(ProcessToObserve::ManagedContainers {
                process_name: proc_to_exec.name.clone(),
                container_names: containers.clone(),
                down: proc_to_exec.down.clone(),
            })
        }

        ProcessType::BareMetal => {
            debug!(
                "Running command {} in detached mode ( Bare metal ) ",
                proc_to_exec.up
            );
            // run the command
            let pid = run_command_detached(&proc_to_exec.up, proc_to_exec.redirect)?;

            // return the pid as a ProcessToObserve
            Ok(ProcessToObserve::ManagedPid {
                process_name: proc_to_exec.name.clone(),
                pid,
                down: proc_to_exec
                    .down
                    .clone()
                    .map(|down| down.replace("{pid}", &pid.to_string())),
            })
        }
    }
}

async fn run_scenario<'a>(
    run_id: i32,
    scenario: &Scenario,
    iteration: i32,
) -> anyhow::Result<iteration::ActiveModel> {
    let start = Utc::now().timestamp_millis();

    // Split the scenario_command into a vector
    let command_parts = match shlex::split(&scenario.command) {
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
    info!("Ran command {}", scenario.command);
    if output.status.success() {
        let stop = Utc::now().timestamp_millis();

        let scenario_iteration = iteration::ActiveModel {
            id: ActiveValue::NotSet,
            run_id: ActiveValue::Set(run_id),
            scenario_name: ActiveValue::Set(scenario.name.clone()),
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
            scenario.command
        ))
    }
}

/// Run a
async fn run_scenarios<'a>(
    run_id: i32,
    scenarios: Vec<&'a Scenario>,
    processes_to_observe: Vec<ProcessToObserve>,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    // ---- for each scenario ----
    for scenario in scenarios {
        // for each iteration
        for iteration in 1..scenario.iterations + 1 {
            println!(
                "> running scenario {} - iteration {}/{}",
                scenario.name.green(),
                iteration,
                scenario.iterations
            );

            // start the metrics loggers
            let stop_handle = metrics_logger::start_logging(processes_to_observe.clone())?;

            // run the scenario
            let scenario_iteration = run_scenario(run_id, &scenario, iteration).await?;
            scenario_iteration.save(db).await?;

            // stop the metrics loggers
            let metrics_log = stop_handle.stop().await?;
            metrics_log.save(run_id, db).await?;
        }
    }

    Ok(())
}

pub async fn run_live<'a>(
    run_id: i32,
    processes_to_observe: Vec<ProcessToObserve>,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    // create a single iteration
    let start = Utc::now().timestamp_millis();
    let iteration = iteration::ActiveModel {
        id: ActiveValue::NotSet,
        run_id: ActiveValue::Set(run_id),
        scenario_name: ActiveValue::Set("live".to_string()),
        count: ActiveValue::Set(1),
        start_time: ActiveValue::Set(start),
        stop_time: ActiveValue::Set(start), // same as start for now, will be updated later
    };
    iteration.save(db).await?;

    // start the metrics logger
    let stop_handle = metrics_logger::start_logging(processes_to_observe.clone())?;

    // keep saving!
    let shared_metrics_log = stop_handle.shared_metrics_log.clone();
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        let shared_metrics_log = shared_metrics_log.clone();
        let mut metrics_log = shared_metrics_log.lock().unwrap();

        metrics_log.save(run_id, &db).await?;
        metrics_log.clear();

        // update the iteration stop time
        let now = Utc::now().timestamp_millis();
        let mut active_iteration = dao::iteration::fetch_live(run_id, &db)
            .await?
            .into_active_model();
        active_iteration.stop_time = ActiveValue::Set(now);
        active_iteration.update(db).await?;

        // update the run stop time
        let now = Utc::now().timestamp_millis();
        let mut active_run = dao::run::fetch(run_id, &db).await?.into_active_model();
        active_run.stop_time = ActiveValue::Set(now);
        active_run.update(db).await?;
    }
}

pub async fn run<'a>(
    exec_plan: ExecutionPlan<'a>,
    db: &DatabaseConnection,
) -> anyhow::Result<DatasetRows> {
    let mut processes_to_observe = exec_plan.external_processes_to_observe.unwrap_or(vec![]); // external procs to observe are cloned here.

    // run the application if there is anything to run
    if !exec_plan.processes_to_execute.is_empty() {
        for proc in exec_plan.processes_to_execute {
            print!("> starting process {}", proc.name.green());

            let process_to_observe = run_process(proc)?;

            // add process_to_observe to the observation list
            processes_to_observe.push(process_to_observe);
            println!("{}", "\t✓".green());
            println!("\t{}", format!("- {}", proc.up).bright_black());
        }
    }

    let start_time = Utc::now().timestamp_millis();
    let is_live = match exec_plan.execution_mode {
        ExecutionMode::Live => true,
        _ => false,
    };

    // check if the processor already exists in the db.
    // If it does then reuse it for this run else save
    // a new one
    let cpu = cpu::Entity::find()
        .filter(cpu::Column::Name.eq(&exec_plan.cpu.name))
        .one(db)
        .await?;

    let cpu_id = match cpu {
        Some(cpu) => cpu.id,
        None => {
            let cpu = match exec_plan.cpu.power {
                Power::Tdp(tdp) => {
                    cpu::ActiveModel {
                        id: ActiveValue::NotSet,
                        name: ActiveValue::Set(exec_plan.cpu.name),
                        tdp: ActiveValue::Set(Some(tdp as f32)),
                        power_curve_id: ActiveValue::NotSet,
                    }
                    .save(db)
                    .await
                }

                Power::Curve(a, b, c, d) => {
                    let power_curve = entities::power_curve::ActiveModel {
                        id: ActiveValue::NotSet,
                        a: ActiveValue::Set(a as f32),
                        b: ActiveValue::Set(b as f32),
                        c: ActiveValue::Set(c as f32),
                        d: ActiveValue::Set(d as f32),
                    }
                    .save(db)
                    .await?
                    .try_into_model()?;

                    cpu::ActiveModel {
                        id: ActiveValue::NotSet,
                        name: ActiveValue::Set(exec_plan.cpu.name),
                        tdp: ActiveValue::NotSet,
                        power_curve_id: ActiveValue::Set(Some(power_curve.id)),
                    }
                    .save(db)
                    .await
                }
            }?;

            cpu.try_into_model()?.id
        }
    };

    // create a new run
    let mut active_run: run::ActiveModel = run::ActiveModel {
        id: ActiveValue::NotSet,
        is_live: ActiveValue::Set(is_live),
        cpu_id: ActiveValue::Set(cpu_id),
        start_time: ActiveValue::Set(start_time),
        stop_time: ActiveValue::set(start_time), // set to start time for now we'll update it later
    }
    .save(db)
    .await?;

    // get the new run id
    let run_id = active_run.clone().try_into_model()?.id;

    // gracefully shutdown upon ctrl-c
    let processes_to_shutdown = processes_to_observe.clone();
    ctrlc::set_handler(move || {
        println!();
        shutdown_application(&processes_to_shutdown)
            .expect("Error shutting down managed processes");
        exit(0)
    })?;

    match exec_plan.execution_mode {
        ExecutionMode::Observation(scenarios) => {
            run_scenarios(run_id, scenarios, processes_to_observe.clone(), db).await?;
        }

        config::ExecutionMode::Live => {
            run_live(run_id, processes_to_observe.clone(), db).await?;
        }
    };

    let stop_time = Utc::now().timestamp_millis(); // Use UTC to avoid confusion, UI can handle
                                                   // timezones

    // update run with the stop time
    active_run.stop_time = ActiveValue::Set(stop_time);
    active_run.save(db).await?;

    // stop the application
    shutdown_application(&processes_to_observe)?;

    // create a dataset containing the data just collected
    Ok(DatasetBuilder::new().scenarios_in_run(run_id).all())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        config::{Process, ProcessType},
        fetch_power, metrics_logger, run_process, ProcessToObserve,
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
            let power = fetch_power(&cpu_name).await?;
            match power {
                Power::Curve(_, _, _, _) => assert!(true),
                Power::Tdp(tdp) => assert!(tdp > 0.0),
            }
            return Ok(());
        }

        panic!()
    }

    #[cfg(target_family = "windows")]
    mod windows {
        use super::*;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "powershell sleep 15".to_string(),
                down: None,
                redirect: None,
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;

            match proc_to_observe {
                ProcessToObserve::ManagedPid {
                    process_name: _,
                    pid,
                    down: _,
                } => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(pid));
                    assert!(proc.is_some());
                }

                _ => panic!("expected to find a process id"),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "powershell sleep 20".to_string(),
                down: None,
                redirect: None,
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;
            let stop_handle = metrics_logger::start_logging(&[&proc_to_observe])?;

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
            let proc = Process {
                name: "sleep".to_string(),
                up: "sleep 15".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;

            match proc_to_observe {
                ProcessToObserve::ManagedPid {
                    process_name,
                    pid,
                    down: _,
                } => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(pid));
                    let proc_name = proc.unwrap().name().to_os_string();
                    let proc_name = proc_name.to_string_lossy();
                    let proc_name = proc_name.deref().to_string();
                    assert!(proc.is_some());
                    assert!(proc_name == process_name);
                }

                e => panic!("expected to find a process id {:?}", e),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "sleep 20".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process_type: ProcessType::BareMetal,
            };
            let procs_to_observe = run_process(&proc)?;
            let stop_handle = metrics_logger::start_logging(vec![procs_to_observe])?;

            tokio::time::sleep(Duration::from_secs(10)).await;

            let metrics_log = stop_handle.stop().await?;

            assert!(!metrics_log.has_errors());
            assert!(!metrics_log.get_metrics().is_empty());

            Ok(())
        }
    }
}
