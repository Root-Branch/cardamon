use anyhow::Context;
use cardamon::{
    cleanup_stdout_stderr,
    config::{self, Config, ExecutionPlan, ProcessToObserve},
    data::{dataset::LiveDataFilter, dataset_builder::DatasetBuilder, Data},
    db_connect, db_migrate, init_config,
    models::rab_model,
    run, server,
};
use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenvy::dotenv;
use itertools::Itertools;
use std::{env, path::Path};
use term_table::{row, row::Row, rows, table_cell::*, Table, TableStyle};
use tracing_subscriber::EnvFilter;
// use textplots::{AxisBuilder, Chart, Plot, Shape, TickDisplay, TickDisplayBuilder};

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Runs a single observation")]
    Run {
        #[arg(help = "Please provide an observation name")]
        name: String,

        #[arg(value_name = "EXTERNAL PIDs", short, long, value_delimiter = ',')]
        pids: Option<Vec<String>>,

        #[arg(
            value_name = "EXTERNAL CONTAINER NAMES",
            short,
            long,
            value_delimiter = ','
        )]
        containers: Option<Vec<String>>,

        #[arg(long)]
        external_only: bool,
    },

    Stats {
        #[arg(
            help = "Please provide a scenario name ('live_<observation name>' for live monitor data)"
        )]
        scenario_name: Option<String>,

        #[arg(value_name = "NUMBER OF PREVIOUS", short = 'n')]
        previous_runs: Option<u64>,
    },

    #[command(about = "Start the Cardamon UI server")]
    Ui {
        #[arg(short, long)]
        port: Option<u32>,
    },

    #[command(about = "Wizard for creating a cardamon.toml file")]
    Init,
}

fn load_config(file: &Option<String>) -> anyhow::Result<Config> {
    // Initialize config if it exists
    match file {
        Some(path) => {
            println!("> using config {}", path.green());
            config::Config::try_from_path(Path::new(path))
        }
        None => {
            println!("> using config {}", "./cardamon.toml".green());
            config::Config::try_from_path(Path::new("./cardamon.toml"))
        }
    }
}

fn add_external_processes(
    pids: Option<Vec<String>>,
    containers: Option<Vec<String>>,
    exec_plan: &mut ExecutionPlan,
) -> anyhow::Result<()> {
    // add external processes to observe.
    for pid in pids.unwrap_or_default() {
        let pid = pid.parse::<u32>()?;
        println!("> including external process {}", pid.to_string().green());
        exec_plan.observe_external_process(ProcessToObserve::ExternalPid(pid));
    }
    if let Some(container_names) = containers {
        exec_plan.observe_external_process(ProcessToObserve::ExternalContainers(container_names));
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // read .env file if it exists
    dotenv().ok();

    // Parse clap args
    let args = Cli::parse();

    let log_filter = env::var("LOG_FILTER").unwrap_or("warn".to_string());

    // Set up tracing subscriber
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(log_filter))
        .with_target(false)
        // .compact()
        .pretty()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // connect to the database and run migrations
    let database_url =
        env::var("DATABASE_URL").unwrap_or("sqlite://cardamon.db?mode=rwc".to_string());
    let database_name = env::var("DATABASE_NAME").unwrap_or("".to_string());
    let db_conn = db_connect(&database_url, Some(&database_name)).await?;
    db_migrate(&db_conn).await?;

    match args.command {
        Commands::Init => {
            init_config().await;
        }

        Commands::Run {
            name,
            pids,
            containers,
            external_only,
        } => {
            println!("\n{}", " Cardamon ".reversed().green());
            let config = load_config(&args.file)
                .context("Error loading configuration, please run `cardamon init`")?;

            // create an execution plan
            let cpu = config.cpu.clone();
            let mut execution_plan = config.create_execution_plan(cpu, &name, external_only)?;

            // add external processes to observe.
            add_external_processes(pids, containers, &mut execution_plan)?;

            // Cleanup previous runs stdout and stderr
            cleanup_stdout_stderr()?;

            // run it!
            let observation_dataset_rows = run(execution_plan, &db_conn).await?;
            let observation_dataset = observation_dataset_rows
                .last_n_runs(5)
                .all()
                .build(&db_conn)
                .await?;

            println!("\n{}", " Summary ".reversed().green());
            for scenario_dataset in observation_dataset
                .by_scenario(LiveDataFilter::ExcludeLive)
                .iter()
            {
                let run_datasets = scenario_dataset.by_run();

                // execute model for current run
                let f = rab_model(0.16);
                let (head, tail) = run_datasets
                    .split_first()
                    .expect("Dataset does not include recent run.");
                let run_data = head.apply_model(&db_conn, &f).await?;

                // execute model for previous runs and calculate trend
                let mut tail_data = vec![];
                for run_dataset in tail {
                    let run_data = run_dataset.apply_model(&db_conn, &f).await?;
                    tail_data.push(run_data.data);
                }
                let tail_data = Data::mean(&tail_data.iter().collect_vec());
                let trend = run_data.data.pow - tail_data.pow;
                let trend_str = match trend.is_nan() {
                    true => "--".bright_black(),
                    false => {
                        if trend > 0.0 {
                            format!("↓ {:.3}Wh", trend).green()
                        } else {
                            format!("↑ {:.3}Wh", trend.abs()).red()
                        }
                    }
                };

                println!(
                    "{}:",
                    format!("{}", scenario_dataset.scenario_name()).green()
                );

                let table = Table::builder()
                    .rows(rows![
                        row![
                            TableCell::builder("Duration (s)".bold()).build(),
                            TableCell::builder("Power (Wh)".bold()).build(),
                            TableCell::builder("CO2 (g)".bold()).build(),
                            TableCell::builder(format!("Trend (over {} runs)", tail.len()).bold())
                                .build()
                        ],
                        row![
                            TableCell::new(format!("{:.3}s", run_data.duration())),
                            TableCell::new(format!("{:.3}Wh", run_data.data.pow)),
                            TableCell::new(format!("{:.3}g", run_data.data.co2)),
                            TableCell::new(trend_str)
                        ]
                    ])
                    .style(TableStyle::rounded())
                    .build();

                println!("{}", table.render())
            }
        }

        Commands::Stats {
            scenario_name,
            previous_runs,
        } => {
            // build dataset
            let dataset_builder = DatasetBuilder::new();
            let dataset_rows = match scenario_name {
                Some(scenario_name) => dataset_builder.scenario(&scenario_name).all(),
                None => dataset_builder.scenarios_all().all(),
            };
            let dataset_cols = match previous_runs {
                Some(n) => dataset_rows.last_n_runs(n).all(),
                None => dataset_rows.runs_all().all(),
            };
            let dataset = dataset_cols.build(&db_conn).await?;

            println!("\n{}", " Cardamon Stats \n".reversed().green());
            if dataset.is_empty() {
                println!("\nno data found!");
            }

            let f = rab_model(0.16);
            for scenario_dataset in dataset.by_scenario(LiveDataFilter::IncludeLive) {
                println!(
                    "Scenario {}:",
                    format!("{}", scenario_dataset.scenario_name()).green()
                );

                let mut table = Table::builder()
                    .rows(rows![row![
                        TableCell::builder("Datetime (Utc)".bold()).build(),
                        TableCell::builder("Duration (s)".bold()).build(),
                        TableCell::builder("Power (Wh)".bold()).build(),
                        TableCell::builder("CO2 (g)".bold()).build()
                    ]])
                    .style(TableStyle::rounded())
                    .build();

                // let mut points: Vec<(f32, f32)> = vec![];
                // let mut run = 0.0;
                for run_dataset in scenario_dataset.by_run() {
                    let run_data = run_dataset.apply_model(&db_conn, &f).await?;
                    let run_start_time = Utc.timestamp_opt(run_data.start_time / 1000, 0).unwrap();
                    let run_duration = (run_data.stop_time - run_data.start_time) as f64 / 1000.0;
                    let _per_min_factor = 60.0 / run_duration;

                    table.add_row(row![
                        TableCell::new(run_start_time.format("%d/%m/%y %H:%M")),
                        TableCell::new(format!("{:.3}s", run_duration)),
                        TableCell::new(format!("{:.4}Wh", run_data.data.pow)),
                        TableCell::new(format!("{:.4}g", run_data.data.co2)),
                    ]);
                    // points.push((run, run_data.data.pow as f32));
                    // run += 1.0;
                }
                println!("{}", table.render());

                // let x_max = points.len() as f32;
                // let y_data = points.iter().map(|(_, y)| *y);
                // let y_min = y_data.clone().reduce(f32::min).unwrap_or(0.0);
                // let y_max = y_data.clone().reduce(f32::max).unwrap_or(0.0);
                //
                // Chart::new_with_y_range(128, 64, 0.0, x_max, y_min, y_max)
                //     .x_axis_style(textplots::LineStyle::Solid)
                //     .y_tick_display(TickDisplay::Sparse)
                //     .lineplot(&Shape::Lines(&points))
                //     .nice();
            }
        }

        Commands::Ui { port } => {
            let port = port.unwrap_or(1337);
            server::start(port, &db_conn).await?
        }
    }

    Ok(())
}
