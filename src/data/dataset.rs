use crate::{
    dao,
    data::Data,
    entities::{iteration::Model as Iteration, metrics::Model as Metrics},
};
use itertools::Itertools;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;

use super::{ProcessData, RunData, ScenarioData};

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
            let proc_id = metric.process_id.clone();
            metrics_by_process
                .entry(proc_id)
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
pub struct Dataset {
    data: Vec<IterationMetrics>,
    pub total_scenarios: u64,
}
impl<'a> Dataset {
    pub fn new(data: Vec<IterationMetrics>, total_scenarios: u64) -> Self {
        Self {
            data,
            total_scenarios,
        }
    }

    pub fn data(&'a self) -> &'a [IterationMetrics] {
        &self.data
    }

    pub fn by_scenario(&'a self) -> Vec<ScenarioDataset<'a>> {
        // get all the scenarios in the dataset
        let scenario_names = self
            .data
            .iter()
            .map(|x| &x.iteration.scenario_name)
            .unique()
            .collect::<Vec<_>>();

        scenario_names
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
            .collect::<Vec<_>>()
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
            // TODO: Check that this is ascending order
            .sorted_by(|a, b| b.iteration.count.cmp(&a.iteration.count))
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
        model: &impl Fn(Vec<&Metrics>, f32) -> Data,
    ) -> anyhow::Result<ScenarioData> {
        let mut all_run_data = vec![];
        for scenario_run_dataset in self.by_run() {
            let run_data = scenario_run_dataset.apply_model(db, model).await?;
            all_run_data.push(run_data);
        }

        let data = Data::sum(
            &all_run_data
                .iter()
                .map(|run_data| &run_data.data)
                .collect_vec(),
        );

        Ok(ScenarioData {
            scenario_name: self.scenario_name.to_string(),
            data,
            run_data: all_run_data,
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
        model: &impl Fn(Vec<&Metrics>, f32) -> Data,
    ) -> anyhow::Result<RunData> {
        let cpu_avg_pow = dao::run::fetch(self.run_id, &db).await?.cpu_avg_power;

        // build up process map
        // proc_id  |  data for proc per iteration
        // =======================================
        // proc_id -> [<data>, <data>]             <- 2 iterations
        // proc_id -> [<data>, <data>]
        let mut proc_iteration_data_map: HashMap<String, Vec<Data>> = HashMap::new();
        for scenario_run_iteration_dataset in self.by_iteration() {
            for (proc_id, metrics) in scenario_run_iteration_dataset.by_process() {
                // run the RAB model to get power and co2 emissions
                let cardamon_data = model(metrics, cpu_avg_pow);

                // if key already exists in map the append cardamon_data to the end of the
                // iteration data vector for that key, else create a new vector for that key.
                let data_vec = match proc_iteration_data_map.get_mut(&proc_id) {
                    Some(data) => {
                        let mut it_data = vec![];
                        it_data.append(data);
                        it_data.push(cardamon_data);
                        it_data
                    }

                    None => vec![cardamon_data],
                };
                proc_iteration_data_map.insert(proc_id.to_string(), data_vec);
            }
        }

        // average data for each process across all iterations
        let proc_data_map: HashMap<String, Data> = proc_iteration_data_map
            .iter()
            .map(|(k, v)| (k, v.iter().collect_vec()))
            .map(|(k, v)| (k.to_string(), Data::mean(&v)))
            .collect();

        // calculate total run data (pow + co2)
        let total_run_data = Data::sum(&proc_data_map.values().collect_vec());

        // convert proc_data_map to vector of ProcessData
        let process_data = proc_data_map
            .into_iter()
            .map(|(process_id, data)| ProcessData { process_id, data })
            .collect_vec();

        Ok(RunData {
            run_id: self.run_id,
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
        data::dataset_builder::DatasetBuilder, db_connect, db_migrate, tests::setup_fixtures,
    };

    #[tokio::test]
    async fn dataset_builder_should_build_a_correct_dataset() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new(&db)
            .scenarios_all()
            .all()
            .last_n_runs(3)
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
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new(&db)
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .await?;

        let scenario_datasets = dataset.by_scenario();
        assert_eq!(scenario_datasets.len(), 3);

        // make sure the scenario names are correct
        let scenario_names = scenario_datasets
            .iter()
            .map(|ds| ds.scenario_name)
            .collect::<Vec<_>>();
        assert_eq!(
            vec!["scenario_1", "scenario_2", "scenario_3"],
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
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new(&db)
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .await?;

        for scenario_dataset in dataset.by_scenario() {
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
                    assert_eq!(vec![1, 2], run_ids);
                }

                "scenario_3" => {
                    assert_eq!(scenario_run_datasets.len(), 3);
                    let run_ids = scenario_run_datasets
                        .iter()
                        .map(|ds| ds.run_id)
                        .collect::<Vec<_>>();
                    assert_eq!(vec![1, 2, 3], run_ids);
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
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let dataset = DatasetBuilder::new(&db)
            .scenarios_all()
            .all()
            .last_n_runs(3)
            .await?;

        for scenario_dataset in dataset.by_scenario() {
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
                        assert_eq!(vec![1, 2], it_ids);
                    }

                    "scenario_3" => {
                        assert_eq!(scenario_run_iteration_datasets.len(), 3);
                        let it_ids = scenario_run_iteration_datasets
                            .iter()
                            .map(|ds| ds.iteration.count)
                            .collect::<Vec<_>>();
                        assert_eq!(vec![1, 2, 3], it_ids);
                    }

                    _ => panic!("unknown scenario in dataset!"),
                }
            }
        }

        Ok(())
    }
}
