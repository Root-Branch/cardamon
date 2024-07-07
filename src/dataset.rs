use crate::data_access::{iteration::Iteration, metrics::Metrics, pagination::Page, DAOService};
use itertools::{Itertools, MinMaxResult};
use std::collections::{hash_map::Entry, HashMap};

pub enum ScenarioSelection {
    All,
    InRun(String),
    InRange { from: i64, to: i64 },
    Search(String),
}

pub enum RunSelection {
    All,
    InRange { from: i64, to: i64 },
}

/// # DatasetBuilder
///
/// The DatasetBuilder allows you to construct a Dataset. There are 2 paths you can follow to build
/// a Dataset which are useful in different uses within Cardamon. These paths exist to stop you from
/// creating an inconsistent Dataset. The sections that follow provide an explaination of each path:
///
///```text
/// [Figure 1 - DatasetBuilder flow]
///
///                         [Single scenario, page runs]
///
///                     ----- DatasetRow ----- DatasetColPager --
///                    |                                         |
/// DatasetBuilder --- +                                         + --- Dataset
///                    |                                         |
///                     -- DatasetRowPager ----- DatasetRows ----
///
///                      [Multiple scenarios, summaries results]
///```
///
/// ## 1 - Single scenario, pagination over runs
///
/// The first creates a Dataset focused on a single scenario and includes some subset of it's most
/// recent runs. This supports the use-case where a user has clicked a single scenario in the UI
/// and wants to view all the times that scenario has been run.
///
/// Example: scenario_runs_by_page("add_10_items", 3, 2)
///  ================================================================================
/// ||  scenarios   || run_1  | run_2  | run_3  |   run_4   |   run_5   |   run_6   ||
/// ||--------------||--------|--------|--------|-----------|-----------|-----------||
/// ||              ||        |        |        | ********************************* ||
/// || add_10_items || <data> | <data> | <data> | * <data>  |  <data>   |  <data> * ||
/// ||              ||        |        |        | ********************************* ||
///  ================================================================================
///
/// ## 2 - Multiple scenarios, 'n' most recent runs_all
///
/// The second creates a Dataset containining some subset of scenarios and the last 'n' times they
/// were run. This is useful when building a summary of a set of scenarios, for example when a user
/// runs cardamon from the CLI.
///
/// Example: last 3 runs of [add_10_items, add_10_users, checkout]
///  ============================================
/// ||  scenarios   || run_1  | run_2  | run_3  ||
/// ||--------------||--------------------------||
/// || add_10_items || <data> | <data> |        ||
/// || add_10_users ||        | <data> | <data> ||
/// || checkout     || <data> |        | <data> ||
///  ============================================
///
///
/// # Example uses
///
/// Example: fetch 3rd page (page size = 5) in runs for add_10_items scenario
///
///```ignore
/// DatasetBuilder::new(&dao_service)
///     .scenario("add_10_items")
///     .runs_all()
///     .page(3, 5)
///     .await?
///```
///
/// Example: fetch the 2nd page of scenarios that match "items" and summarise the last 5 runs
///
/// ```ignore
/// DatasetBuilder::new(&dao_service)
///     .scenarios_by_name("items")
///     .page(2, 5)
///     .last_n_runs(5)
///     .await?
///```
///
pub struct DatasetBuilder<'a> {
    dao_service: &'a dyn DAOService,
}
impl<'a> DatasetBuilder<'a> {
    pub fn new(dao_service: &'a dyn DAOService) -> Self {
        Self { dao_service }
    }

    /// Returns a single scenario.
    pub fn scenario(&self, scenario: &str) -> DatasetRow {
        DatasetRow {
            scenario: scenario.to_string(),
            dao_service: self.dao_service,
        }
    }

    /// Returns all scenarios.
    pub fn scenarios_all(&self) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::All,
            dao_service: self.dao_service,
        }
    }

    /// Returns all scenarios that were executed in a single run.
    pub fn scenarios_in_run(&self, run: &str) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::InRun(run.to_string()),
            dao_service: self.dao_service,
        }
    }

    /// Returns all scenarios that were executed at some time within the given time range.
    ///
    /// * Arguments
    /// - from: unix timestamp in millis
    /// - to: unix timestamp n millis
    pub fn scenarios_in_range(&self, from: i64, to: i64) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::InRange { from, to },
            dao_service: self.dao_service,
        }
    }

    /// Returns a DatasetRowPager all scenarios that match the given name. This function does not fetch these
    /// scenarios, it just defines the maximum set of scenarios which can be filtered in subsequent
    /// steps.
    pub fn scenarios_by_name(&self, name: &str) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::Search(name.to_string()),
            dao_service: self.dao_service,
        }
    }
}

/// The DatasetRowPager defines an incomplete Dataset which includes set of scenarios (rows)
/// without any runs.
///
/// It provides functions to select a subset within that range of scenarios.
pub struct DatasetRowPager<'a> {
    scenario_selection: ScenarioSelection,
    dao_service: &'a dyn DAOService,
}
impl<'a> DatasetRowPager<'a> {
    /// Returns a DatasetRows object which defined the full set of scenarios defined by this
    /// DatasetRowPager.
    pub fn all(self) -> DatasetRows<'a> {
        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: None,
            dao_service: self.dao_service,
        }
    }

    /// Returns a DatasetRows object which defines a subset of the scenarios defined by this
    /// DatasetRowPager.
    pub fn page(self, page_size: u32, page_num: u32) -> DatasetRows<'a> {
        let scenario_page = Page {
            size: page_size,
            num: page_num,
        };

        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: Some(scenario_page),
            dao_service: self.dao_service,
        }
    }
}

/// The DatasetRows defines an incomplete Dataet defining a set of scenarios (rows) without any
/// runs. This contains an optional Page object which defines some subset of this set of scenarios.
/// If no Page is provided then DatasetRows defines the full range of scenarios instead of a single
/// page within it.
///
/// Example: page 2 (page size = 2) of the rows containing 4 scenarios.
///  ================================
/// ||     scenarios    || runs ... ||
/// ||------------------||----------||
/// ||   add_10_items   ||          ||
/// ||   add_10_users   ||          ||
/// || **************** ||    ...   ||
/// || * checkout     * ||          ||
/// || * search_item  * ||          ||
/// || **************** ||          ||
///  ================================
///
pub struct DatasetRows<'a> {
    scenario_selection: ScenarioSelection,
    scenario_page: Option<Page>,
    dao_service: &'a dyn DAOService,
}
impl<'a> DatasetRows<'a> {
    /// Returns a Dataset which contains the iterations and metrics collected in the last 'n' runs
    /// of each scenario.
    ///
    /// This function is async as it uses the dao_service to fetch the results from the db.
    pub async fn last_n_runs(&self, n: u32) -> anyhow::Result<Dataset> {
        let scenarios = match &self.scenario_selection {
            ScenarioSelection::All => {
                self.dao_service
                    .scenarios()
                    .fetch_all(&self.scenario_page)
                    .await
            }
            ScenarioSelection::Search(name) => {
                self.dao_service
                    .scenarios()
                    .fetch_by_name(name, &self.scenario_page)
                    .await
            }
            ScenarioSelection::InRun(run) => {
                self.dao_service
                    .scenarios()
                    .fetch_in_run(run, &self.scenario_page)
                    .await
            }
            ScenarioSelection::InRange { from, to } => {
                self.dao_service
                    .scenarios()
                    .fetch_in_range(*from, *to, &self.scenario_page)
                    .await
            }
        }?;

        // for each scenario get the associated iterations in the last n runs
        let mut iterations = vec![];
        for scenario in scenarios {
            let scenario_iterations = self
                .dao_service
                .iterations()
                .fetch_runs_last_n(&scenario, n)
                .await?;
            iterations.extend(scenario_iterations);
        }

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics = self
                .dao_service
                .metrics()
                .fetch_within(&it.run_id, it.start_time, it.stop_time)
                .await?;
            iterations_with_metrics.push(IterationWithMetrics::new(it, metrics));
        }

        // TODO: cache the iterations/metrics data

        Ok(Dataset::new(iterations_with_metrics))
    }
}

/// The DatasetRow defines an incomplete Dataset with a single scenario (row) without any runs.
/// This object provides functions for defining a range of runs to include for the scenario.
pub struct DatasetRow<'a> {
    scenario: String,
    dao_service: &'a dyn DAOService,
}
impl<'a> DatasetRow<'a> {
    /// Return a DataColPager which includes all the runs for this scenario.
    pub fn runs_all(self) -> DatasetColPager<'a> {
        DatasetColPager {
            scenario: self.scenario,
            run_selection: RunSelection::All,
            dao_service: self.dao_service,
        }
    }

    /// Return a DatasetColPager which includes only those runs which were executed within the
    /// given time range.
    ///
    /// * Arguments
    /// - from: unix timestamp in millis
    /// - to: unix timestamp in millis
    pub fn runs_in_range(self, from: i64, to: i64) -> DatasetColPager<'a> {
        DatasetColPager {
            scenario: self.scenario,
            run_selection: RunSelection::InRange { from, to },
            dao_service: self.dao_service,
        }
    }
}

/// The DatasetColPager defines an incomplete Dataset which includes a single scenario (row) and
/// range of runs for that scenario.
///
/// It provides a single function to select a single page within that range of runs.
pub struct DatasetColPager<'a> {
    scenario: String,
    run_selection: RunSelection,
    dao_service: &'a dyn DAOService,
}
impl<'a> DatasetColPager<'a> {
    pub async fn page(&self, page_size: u32, page_num: u32) -> anyhow::Result<Dataset> {
        let page = Page::new(page_size, page_num);

        let iterations = match self.run_selection {
            RunSelection::All => {
                self.dao_service
                    .iterations()
                    .fetch_runs_all(&self.scenario, &page)
                    .await
            }

            RunSelection::InRange { from, to } => {
                self.dao_service
                    .iterations()
                    .fetch_runs_in_range(&self.scenario, from, to, &page)
                    .await
            }
        }?;

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics = self
                .dao_service
                .metrics()
                .fetch_within(&it.run_id, it.start_time, it.stop_time)
                .await?;
            iterations_with_metrics.push(IterationWithMetrics::new(it, metrics));
        }

        // TODO: cache the iterations/metrics data
        //

        Ok(Dataset::new(iterations_with_metrics))
    }
}

// ////////////////////////////////////////////////////////////////////////////////////////////////
//  DATASET
//  ///////////////////////////////////////////////////////////////////////////////////////////////

/// Associates a single ScenarioIteration with all the metrics captured for it.
#[derive(Debug)]
pub struct IterationWithMetrics {
    iteration: Iteration,
    metrics: Vec<Metrics>,
}
impl IterationWithMetrics {
    pub fn new(iteration: Iteration, metrics: Vec<Metrics>) -> Self {
        Self { iteration, metrics }
    }

    pub fn iteration(&self) -> &Iteration {
        &self.iteration
    }

    pub fn metrics(&self) -> &[Metrics] {
        &self.metrics
    }

    pub fn accumulate_by_process(&self) -> Vec<ProcessMetrics> {
        let mut metrics_by_process: HashMap<String, Vec<&Metrics>> = HashMap::new();
        for metric in self.metrics.iter() {
            let proc_id = metric.process_id.clone();
            metrics_by_process
                .entry(proc_id)
                .and_modify(|v| v.push(metric))
                .or_insert(vec![metric]); // if entry doesn't exist then create a new vec
        }

        metrics_by_process
            .into_iter()
            .map(|(process_id, metrics)| {
                let cpu_usage_minmax = metrics.iter().map(|m| m.cpu_usage).minmax();
                let cpu_usage_total = metrics.iter().fold(0.0, |acc, m| acc + m.cpu_usage);
                let cpu_usage_mean = cpu_usage_total / metrics.len() as f64;

                ProcessMetrics {
                    process_id,
                    cpu_usage_minmax,
                    cpu_usage_mean,
                    cpu_usage_total,
                }
            })
            .collect()
    }
}

/// Read-only struct containing metrics for a single process.
#[derive(Debug)]
pub struct ProcessMetrics {
    process_id: String,
    cpu_usage_minmax: MinMaxResult<f64>,
    cpu_usage_mean: f64,
    cpu_usage_total: f64,
}
impl ProcessMetrics {
    pub fn process_id(&self) -> &str {
        &self.process_id
    }

    pub fn cpu_usage_minmax(&self) -> &MinMaxResult<f64> {
        &self.cpu_usage_minmax
    }

    pub fn cpu_usage_mean(&self) -> f64 {
        self.cpu_usage_mean
    }

    pub fn cpu_usage_total(&self) -> f64 {
        self.cpu_usage_total
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
    data: Vec<IterationWithMetrics>,
}
impl<'a> Dataset {
    pub fn new(data: Vec<IterationWithMetrics>) -> Self {
        Self { data }
    }

    pub fn data(&'a self) -> &'a [IterationWithMetrics] {
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
    data: Vec<&'a IterationWithMetrics>,
}
impl<'a> ScenarioDataset<'a> {
    pub fn scenario_name(&'a self) -> &'a str {
        self.scenario_name
    }

    pub fn data(&'a self) -> &'a [&'a IterationWithMetrics] {
        &self.data
    }

    pub fn by_run(&'a self) -> Vec<RunDataset<'a>> {
        let runs = self
            .data
            .iter()
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

                RunDataset {
                    scenario_name: self.scenario_name,
                    run_id,
                    data,
                }
            })
            .collect::<Vec<_>>()
    }
}

/// Dataset containing data associated with a single scenario in a single cardamon run but
/// potentially containing data taken from multiple scenario iterations.
///
/// Guarenteed to contain only data associated with a single scenario and cardamon run.
#[derive(Debug)]
pub struct RunDataset<'a> {
    scenario_name: &'a str,
    run_id: &'a str,
    data: Vec<&'a IterationWithMetrics>,
}
impl<'a> RunDataset<'a> {
    pub fn scenario_name(&'a self) -> &'a str {
        self.scenario_name
    }

    pub fn run_id(&'a self) -> &'a str {
        self.run_id
    }

    pub fn by_iterations(&'a self) -> &'a [&'a IterationWithMetrics] {
        &self.data
    }

    pub fn averaged(&'a self) -> Vec<ProcessMetrics> {
        let all_process_metrics = self
            .data
            .iter()
            .flat_map(|i| i.accumulate_by_process())
            .collect::<Vec<_>>();

        let mut process_metrics_to_iterations: HashMap<String, Vec<ProcessMetrics>> =
            HashMap::new();
        for process_metrics in all_process_metrics.into_iter() {
            let proc_id = process_metrics.process_id.clone();
            let entry = process_metrics_to_iterations.entry(proc_id);
            match entry {
                Entry::Occupied(_) => {
                    entry.and_modify(|v| v.push(process_metrics));
                }
                Entry::Vacant(_) => {
                    entry.or_insert(vec![process_metrics]);
                }
            }
        }

        // average across iterations
        process_metrics_to_iterations
            .into_iter()
            .flat_map(|(_, process_metrics)| {
                process_metrics.into_iter().reduce(|a, b| {
                    let a_minmax = match a.cpu_usage_minmax {
                        MinMaxResult::NoElements => None,
                        MinMaxResult::OneElement(val) => Some((val, val)),
                        MinMaxResult::MinMax(min, max) => Some((min, max)),
                    };
                    let b_minmax = match b.cpu_usage_minmax {
                        MinMaxResult::NoElements => None,
                        MinMaxResult::OneElement(val) => Some((val, val)),
                        MinMaxResult::MinMax(min, max) => Some((min, max)),
                    };

                    let cpu_usage_minmax = if a_minmax.is_some() && b_minmax.is_some() {
                        let (a_min, a_max) = a_minmax.unwrap();
                        let (b_min, b_max) = b_minmax.unwrap();
                        MinMaxResult::MinMax(a_min + b_min / 2.0, a_max + b_max / 2.0)
                    } else {
                        MinMaxResult::NoElements
                    };

                    ProcessMetrics {
                        process_id: a.process_id,
                        cpu_usage_minmax,
                        cpu_usage_mean: a.cpu_usage_mean + b.cpu_usage_mean / 2.0,
                        cpu_usage_total: a.cpu_usage_total + b.cpu_usage_total / 2.0,
                    }
                })
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    //     use crate::data_access::{DataAccessService, LocalDataAccessService};
    //     use sqlx::SqlitePool;
    //
    //     #[sqlx::test(
    //         migrations = "./migrations",
    //         fixtures(
    //             "../fixtures/runs.sql",
    //             "../fixtures/scenario_iterations.sql",
    //             "../fixtures/cpu_metrics.sql"
    //         )
    //     )]
    //     async fn datasets_work(pool: SqlitePool) -> anyhow::Result<()> {
    //         let data_access_service = LocalDataAccessService::new(pool.clone());
    //         let observation_dataset = data_access_service
    //             .fetch_observation_dataset(vec!["scenario_2"], 2)
    //             .await?;
    //
    //         assert_eq!(observation_dataset.data().len(), 4);
    //
    //         let scenario_datasets = observation_dataset.by_scenario();
    //         assert_eq!(scenario_datasets.len(), 1);
    //
    //         for scenario_dataset in scenario_datasets.iter() {
    //             // println!("{:?}", scenario_dataset);
    //             let run_datasets = scenario_dataset.by_run();
    //             assert_eq!(run_datasets.len(), 2);
    //
    //             for run_dataset in run_datasets.iter() {
    //                 // println!("{:?}", run_dataset);
    //                 let avg = run_dataset.averaged();
    //                 assert_eq!(avg.len(), 2);
    //             }
    //         }
    //
    //         pool.close().await;
    //         Ok(())
    //     }
}
