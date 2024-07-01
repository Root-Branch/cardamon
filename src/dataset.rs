use crate::data_access::{cpu_metrics::CpuMetrics, scenario_iteration::ScenarioIteration};
use itertools::{Itertools, MinMaxResult};
use std::collections::{hash_map::Entry, HashMap};

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

/// Associates a single ScenarioIteration with all the metrics captured for it.
#[derive(Debug)]
pub struct IterationWithMetrics {
    scenario_iteration: ScenarioIteration,
    cpu_metrics: Vec<CpuMetrics>,
}
impl IterationWithMetrics {
    pub fn new(scenario_it: ScenarioIteration, cpu_metrics: Vec<CpuMetrics>) -> Self {
        Self {
            scenario_iteration: scenario_it,
            cpu_metrics,
        }
    }

    pub fn scenario_iteration(&self) -> &ScenarioIteration {
        &self.scenario_iteration
    }

    pub fn cpu_metrics(&self) -> &[CpuMetrics] {
        &self.cpu_metrics
    }

    pub fn accumulate_by_process(&self) -> Vec<ProcessMetrics> {
        let mut metrics_by_process: HashMap<String, Vec<&CpuMetrics>> = HashMap::new();
        for metric in self.cpu_metrics.iter() {
            let proc_id = metric.process_id.clone();
            metrics_by_process
                .entry(proc_id)
                .and_modify(|v| v.push(metric))
                .or_insert(vec![metric]); // if entry doesn't exist then create a new vec
        }

        metrics_by_process
            .into_iter()
            .map(|(process_id, cpu_metrics)| {
                let cpu_usage_minmax = cpu_metrics.iter().map(|m| m.cpu_usage).minmax();
                let cpu_usage_total = cpu_metrics.iter().fold(0.0, |acc, m| acc + m.cpu_usage);
                let cpu_usage_mean = cpu_usage_total / cpu_metrics.len() as f64;

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

/// Dataset potentially containing multiple scenarios with multiple iterations across multiple
/// cardamon runs.
pub struct ObservationDataset {
    data: Vec<IterationWithMetrics>,
}
impl<'a> ObservationDataset {
    pub fn new(data: Vec<IterationWithMetrics>) -> Self {
        Self { data }
    }

    pub fn data(&'a self) -> &'a [IterationWithMetrics] {
        &self.data
    }

    pub fn by_scenario(&'a self) -> Vec<ScenarioDataset<'a>> {
        // get all the scenarios in the observation
        let scenario_names = self
            .data
            .iter()
            .map(|x| &x.scenario_iteration.scenario_name)
            .unique()
            .collect::<Vec<_>>();

        scenario_names
            .into_iter()
            .map(|scenario_name| {
                let data = self
                    .data
                    .iter()
                    .filter(|x| &x.scenario_iteration.scenario_name == scenario_name)
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
            .map(|x| &x.scenario_iteration.run_id)
            .unique()
            .collect::<Vec<_>>();

        runs.into_iter()
            .map(|run_id| {
                let data = self
                    .data
                    .iter()
                    .filter(|x| &x.scenario_iteration.run_id == run_id)
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
    use crate::data_access::{DataAccessService, LocalDataAccessService};
    use sqlx::SqlitePool;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures(
            "../fixtures/runs.sql",
            "../fixtures/scenario_iterations.sql",
            "../fixtures/cpu_metrics.sql"
        )
    )]
    async fn datasets_work(pool: SqlitePool) -> anyhow::Result<()> {
        let data_access_service = LocalDataAccessService::new(pool.clone());
        let observation_dataset = data_access_service
            .fetch_observation_dataset(vec!["scenario_2"], 2)
            .await?;

        assert_eq!(observation_dataset.data().len(), 4);

        let scenario_datasets = observation_dataset.by_scenario();
        assert_eq!(scenario_datasets.len(), 1);

        for scenario_dataset in scenario_datasets.iter() {
            // println!("{:?}", scenario_dataset);
            let run_datasets = scenario_dataset.by_run();
            assert_eq!(run_datasets.len(), 2);

            for run_dataset in run_datasets.iter() {
                // println!("{:?}", run_dataset);
                let avg = run_dataset.averaged();
                assert_eq!(avg.len(), 2);
            }
        }

        pool.close().await;
        Ok(())
    }
}
