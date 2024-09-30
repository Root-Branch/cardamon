use crate::{
    config::Power,
    dao::{self, pagination::Pages},
    data::Data,
    entities::{self, iteration::Model as Iteration, metrics::Model as Metrics},
};
use anyhow::Context;
use itertools::Itertools;
use sea_orm::{DatabaseConnection, ModelTrait};
use std::collections::HashMap;

use super::{ProcessData, ProcessMetrics, RunData, ScenarioData};

pub enum AggregationMethod {
    MostRecent,
    Average,
    Sum,
}

pub enum LiveDataFilter {
    IncludeLive,
    ExcludeLive,
    OnlyLive,
}

/// Associates a single ScenarioIteration with all the metrics captured for it.
#[derive(Debug)]
pub struct IterationMetrics {
    iteration: Iteration,
    metrics: Vec<Metrics>,
}
impl IterationMetrics {
    pub fn new(iteration: Iteration, metrics: Vec<Metrics>) -> Self {
        Self { iteration, metrics }
    }

    pub fn iteration(&self) -> &Iteration {
        &self.iteration
    }

    pub fn metrics(&self) -> &[Metrics] {
        &self.metrics
    }

    pub fn by_process(&self) -> HashMap<String, Vec<&Metrics>> {
        let mut metrics_by_process: HashMap<String, Vec<&Metrics>> = HashMap::new();
        for metric in self.metrics.iter() {
            let proc_name = metric.process_name.clone();
            metrics_by_process
                .entry(proc_name)
                .and_modify(|v| v.push(metric))
                .or_insert(vec![metric]); // if entry doesn't exist then create a new vec
        }

        metrics_by_process
    }
}

/// Data in cardamon is organised as a table. Each row is a scenario and each column is a run
/// of that scenario.
///
/// Example: Dataset containing the most recent 3 runs of 3 different scenarios.
///  ============================================
/// ||  scenarios   || run_1  | run_2  | run_3  ||
/// ||--------------||--------------------------||
/// || add_10_items || <data> | <data> |        ||
/// || add_10_users ||        | <data> | <data> ||
/// || checkout     || <data> |        | <data> ||
///  ============================================
///
/// Example: Dataset containing the 2nd page of runs for the `add_10_items` scenario.
///  ================================================================================
/// ||  scenarios   || run_1  | run_2  | run_3  |   run_4   |   run_5   |   run_6   ||
/// ||--------------||--------|--------|--------|-----------|-----------|-----------||
/// ||              ||        |        |        | ********************************* ||
/// || add_10_items || <data> | <data> | <data> | * <data>  |  <data>   |  <data> * ||
/// ||              ||        |        |        | ********************************* ||
///  ================================================================================
///
#[derive(Debug)]
pub struct Dataset {
    data: Vec<IterationMetrics>,
    pub total_scenarios: Pages,
    pub total_runs: Pages,
}
impl<'a> Dataset {
    pub fn new(data: Vec<IterationMetrics>, total_scenarios: Pages, total_runs: Pages) -> Self {
        Self {
            data,
            total_scenarios,
            total_runs,
        }
    }

    pub fn data(&'a self) -> &'a [IterationMetrics] {
        &self.data
    }

    pub fn is_empty(&'a self) -> bool {
        self.data.is_empty()
    }

    pub fn by_scenario(&'a self, live_data_filter: LiveDataFilter) -> Vec<ScenarioDataset<'a>> {
        // get all the scenarios in the dataset
        let unique_scenario_names = self
            .data
            .iter()
            // .sorted_by(|a, b| b.iteration.start_time.cmp(&a.iteration.start_time))
            .map(|x| &x.iteration.scenario_name)
            .unique();

        // let poop = unique_scenario_names.clone().collect_vec();
        // println!("unique names = {:?}", poop);
        let scenario_names = match live_data_filter {
            LiveDataFilter::IncludeLive => unique_scenario_names.collect_vec(),
            LiveDataFilter::ExcludeLive => unique_scenario_names
                .filter(|name| !name.starts_with("live"))
                .collect_vec(),
            LiveDataFilter::OnlyLive => unique_scenario_names
                .filter(|name| name.starts_with("live"))
                .collect_vec(),
        };

        let poopy = scenario_names
            .into_iter()
            .map(|scenario_name| {
                let data = self
                    .data
                    .iter()
                    .filter(|x| &x.iteration.scenario_name == scenario_name)
                    .collect::<Vec<_>>();

                ScenarioDataset {
                    scenario_name,
                    data,
                }
            })
            .collect::<Vec<_>>();

        // println!("poopy = {:?}", poopy);
        poopy
    }
}

/// Dataset containing data associated with a single scenario but potentially containing data
/// taken from multiple cardamon runs.
///
/// Guarenteed to contain only data associated with a single scenario.
#[derive(Debug)]
pub struct ScenarioDataset<'a> {
    scenario_name: &'a str,
    data: Vec<&'a IterationMetrics>,
}
impl<'a> ScenarioDataset<'a> {
    pub fn scenario_name(&'a self) -> &'a str {
        self.scenario_name
    }

    pub fn data(&'a self) -> &'a [&'a IterationMetrics] {
        &self.data
    }

    pub fn by_run(&'a self) -> Vec<ScenarioRunDataset<'a>> {
        let runs = self
            .data
            .iter()
            .sorted_by(|a, b| b.iteration.start_time.cmp(&a.iteration.start_time))
            .map(|x| &x.iteration.run_id)
            .unique()
            .collect::<Vec<_>>();

        runs.into_iter()
            .map(|run_id| {
                let data = self
                    .data
                    .iter()
                    .filter(|x| &x.iteration.run_id == run_id)
                    .cloned()
                    .collect::<Vec<_>>();

                ScenarioRunDataset {
                    scenario_name: self.scenario_name,
                    run_id: *run_id,
                    data,
                }
            })
            .collect::<Vec<_>>()
    }

    pub async fn apply_model(
        &'a self,
        db: &DatabaseConnection,
        model: &impl Fn(&Vec<&Metrics>, &Power) -> Data,
        aggregation_method: AggregationMethod,
    ) -> anyhow::Result<ScenarioData> {
        let mut all_run_data = vec![];
        for scenario_run_dataset in self.by_run() {
            let run_data = scenario_run_dataset.apply_model(db, model).await?;
            all_run_data.push(run_data);
        }

        // use the aggregation method to calculate the data for this scenario
        let data = match aggregation_method {
            AggregationMethod::MostRecent => all_run_data.first().context("no data!")?.data.clone(),

            AggregationMethod::Average => Data::mean(
                &all_run_data
                    .iter()
                    .map(|run_data| &run_data.data)
                    .collect_vec(),
            ),

            AggregationMethod::Sum => Data::sum(
                &all_run_data
                    .iter()
                    .map(|run_data| &run_data.data)
                    .collect_vec(),
            ),
        };

        // calculate trend
        let mut delta_sum = 0_f64;
        let mut delta_sum_abs = 0_f64;
        for i in 0..all_run_data.len() - 1 {
            let delta = all_run_data[i + 1].data.pow - all_run_data[i].data.pow;
            delta_sum += delta;
            delta_sum_abs += delta.abs();
        }

        Ok(ScenarioData {
            scenario_name: self.scenario_name.to_string(),
            data,
            run_data: all_run_data,
            trend: if delta_sum_abs != 0_f64 {
                delta_sum / delta_sum_abs
            } else {
                0_f64
            },
        })
    }
}

/// Dataset containing data associated with a single scenario in a single cardamon run but
/// potentially containing data taken from multiple scenario iterations.
///
/// Guarenteed to contain only data associated with a single scenario and cardamon run.
#[derive(Debug)]
pub struct ScenarioRunDataset<'a> {
    scenario_name: &'a str,
    run_id: i32,
    data: Vec<&'a IterationMetrics>,
}
impl<'a> ScenarioRunDataset<'a> {
    pub fn scenario_name(&'a self) -> &'a str {
        self.scenario_name
    }

    pub fn run_id(&'a self) -> i32 {
        self.run_id
    }

    pub fn data(&'a self) -> &'a [&'a IterationMetrics] {
        &self.data
    }

    pub fn by_iteration(&'a self) -> ScenarioRunIterationDataset {
        &self.data
    }

    pub async fn apply_model(
        &'a self,
        db: &DatabaseConnection,
        model: &impl Fn(&Vec<&Metrics>, &Power) -> Data,
    ) -> anyhow::Result<RunData> {
        let run = dao::run::fetch(self.run_id, &db).await?;
        let cpu = run
            .find_related(entities::cpu::Entity)
            .one(db)
            .await?
            .context("Run is missing CPU!")?;
        let power = cpu
            .find_related(entities::power_curve::Entity)
            .one(db)
            .await?
            .map(|power| {
                Power::Curve(
                    power.a as f64,
                    power.b as f64,
                    power.c as f64,
                    power.d as f64,
                )
            })
            .or(cpu.tdp.map(|tdp| Power::Tdp(tdp as f64)))
            .context("Run is missing CPU or CPU is missing power")?;

        let start_time = run.start_time;
        let stop_time = run.stop_time;

        // build up process map
        // proc_id  |  data & metrics per iteration for proc per iteration
        // =======================================
        // proc_id -> [<(data, [metrics)>, <(data, metrics)>]    <- 2 iterations
        // proc_id -> [<(data, metrics)>, <(data, metrics)>]    <- 2 iterations
        let mut proc_iteration_data_map: HashMap<String, (Vec<Data>, Vec<Vec<ProcessMetrics>>)> =
            HashMap::new();
        for scenario_run_iteration_dataset in self.by_iteration() {
            for (proc_id, metrics) in scenario_run_iteration_dataset.by_process() {
                // run the RAB model to get power and co2 emissions
                let cardamon_data = model(&metrics, &power);

                // convert the metrics database model into metrics data
                let proc_metrics = metrics
                    .iter()
                    .map(|metrics| ProcessMetrics {
                        proc_id: proc_id.clone(),
                        timestamp: metrics.time_stamp,
                        cpu_usage: metrics.cpu_usage,
                    })
                    .collect_vec();

                // if key already exists in map the append cardamon_data to the end of the
                // iteration data vector for that key, else create a new vector for that key.
                let data_vec = match proc_iteration_data_map.get_mut(&proc_id) {
                    Some((proc_data, iteration_metrics)) => {
                        let mut data = vec![];
                        data.append(proc_data);
                        data.push(cardamon_data);

                        let mut metrics = vec![];
                        metrics.append(iteration_metrics);
                        metrics.push(proc_metrics);

                        (data, metrics)
                    }

                    None => (vec![cardamon_data], vec![proc_metrics]),
                };
                proc_iteration_data_map.insert(proc_id.to_string(), data_vec);
            }
        }

        // average data for each process across all iterations
        let proc_data_map: HashMap<String, (Data, Vec<Vec<ProcessMetrics>>)> =
            proc_iteration_data_map
                .into_iter()
                .map(|(k, (data, metrics))| {
                    (
                        k.to_string(),
                        (Data::mean(&data.iter().collect_vec()), metrics),
                    )
                })
                .collect();

        // calculate total run data (pow + co2)
        let total_run_data = Data::sum(&proc_data_map.values().map(|(data, _)| data).collect_vec());

        // convert proc_data_map to vector of ProcessData
        let process_data = proc_data_map
            .into_iter()
            .map(|(process_id, (data, iteration_metrics))| ProcessData {
                process_id,
                pow_perc: data.pow / total_run_data.pow,
                data,
                iteration_metrics,
            })
            .collect_vec();

        Ok(RunData {
            run_id: self.run_id,
            start_time,
            stop_time,
            data: total_run_data,
            process_data,
        })
    }
}

type ScenarioRunIterationDataset<'a> = &'a [&'a IterationMetrics];

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{
        data::{dataset::LiveDataFilter, dataset_builder::DatasetBuilder},
        db_connect, db_migrate,
        tests::setup_fixtures,
    };

    #[tokio::test]
    async fn dataset_builder_should_build_a_correct_dataset() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .all()
            .build(&db)
            .await?;

        assert_eq!(dataset.data.len(), 14);

        Ok(())
    }

    #[tokio::test]
    async fn dataset_can_be_broken_down_to_scenario_datasets() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .all()
            .build(&db)
            .await?;

        let scenario_datasets = dataset.by_scenario(LiveDataFilter::ExcludeLive);
        assert_eq!(scenario_datasets.len(), 3);

        // make sure the scenario names are correct
        let scenario_names = scenario_datasets
            .iter()
            .map(|ds| ds.scenario_name)
            .collect::<Vec<_>>();
        assert_eq!(
            vec!["scenario_3", "scenario_2", "scenario_1"],
            scenario_names
        );

        // make sure the data in the datasets are correct
        for scenario_dataset in scenario_datasets {
            match scenario_dataset.scenario_name {
                "scenario_1" => {
                    assert_eq!(scenario_dataset.data.len(), 1);
                    assert!(
                        scenario_dataset
                            .data
                            .iter()
                            .flat_map(|x| &x.metrics)
                            .collect_vec()
                            .len()
                            == 10
                    );
                }

                "scenario_2" => {
                    assert_eq!(scenario_dataset.data.len(), 4);
                    assert!(
                        scenario_dataset
                            .data
                            .iter()
                            .flat_map(|x| &x.metrics)
                            .collect_vec()
                            .len()
                            == 40
                    );
                }

                "scenario_3" => {
                    assert_eq!(scenario_dataset.data.len(), 9);
                    assert!(
                        scenario_dataset
                            .data
                            .iter()
                            .flat_map(|x| &x.metrics)
                            .collect_vec()
                            .len()
                            == 90
                    );
                }

                _ => panic!("Unknown scenario in dataset"),
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn scenario_dataset_can_be_broken_down_to_scenario_run_datasets() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .all()
            .build(&db)
            .await?;

        for scenario_dataset in dataset.by_scenario(LiveDataFilter::ExcludeLive) {
            let scenario_run_datasets = scenario_dataset.by_run();

            match scenario_dataset.scenario_name {
                "scenario_1" => {
                    assert_eq!(scenario_run_datasets.len(), 1);
                    let run_ids = scenario_run_datasets
                        .iter()
                        .map(|ds| ds.run_id)
                        .collect::<Vec<_>>();
                    assert_eq!(vec![1], run_ids);
                }

                "scenario_2" => {
                    assert_eq!(scenario_run_datasets.len(), 2);
                    let run_ids = scenario_run_datasets
                        .iter()
                        .map(|ds| ds.run_id)
                        .collect::<Vec<_>>();
                    assert_eq!(vec![2, 1], run_ids);
                }

                "scenario_3" => {
                    assert_eq!(scenario_run_datasets.len(), 3);
                    let run_ids = scenario_run_datasets
                        .iter()
                        .map(|ds| ds.run_id)
                        .collect::<Vec<_>>();
                    assert_eq!(vec![3, 2, 1], run_ids);
                }

                _ => panic!("unknown scenario in dataset!"),
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn scenario_run_dataset_can_be_broken_down_to_scenario_run_iteration_datasets(
    ) -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .all()
            .build(&db)
            .await?;

        for scenario_dataset in dataset.by_scenario(LiveDataFilter::ExcludeLive) {
            for scenario_run_dataset in scenario_dataset.by_run() {
                let scenario_run_iteration_datasets = scenario_run_dataset.by_iteration();

                match scenario_dataset.scenario_name {
                    "scenario_1" => {
                        assert_eq!(scenario_run_iteration_datasets.len(), 1);
                        let it_ids = scenario_run_iteration_datasets
                            .iter()
                            .map(|ds| ds.iteration.count)
                            .collect::<Vec<_>>();
                        assert_eq!(vec![1], it_ids);
                    }

                    "scenario_2" => {
                        assert_eq!(scenario_run_iteration_datasets.len(), 2);
                        let it_ids = scenario_run_iteration_datasets
                            .iter()
                            .map(|ds| ds.iteration.count)
                            .collect::<Vec<_>>();
                        assert_eq!(vec![2, 1], it_ids);
                    }

                    "scenario_3" => {
                        assert_eq!(scenario_run_iteration_datasets.len(), 3);
                        let it_ids = scenario_run_iteration_datasets
                            .iter()
                            .map(|ds| ds.iteration.count)
                            .collect::<Vec<_>>();
                        assert_eq!(vec![3, 2, 1], it_ids);
                    }

                    _ => panic!("unknown scenario in dataset!"),
                }
            }
        }

        Ok(())
    }
}
