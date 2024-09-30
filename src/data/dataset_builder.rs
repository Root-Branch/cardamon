use crate::{
    dao::{
        self,
        pagination::{Page, Pages},
    },
    data::dataset::{Dataset, IterationMetrics},
};
use anyhow::Context;
use sea_orm::DatabaseConnection;
use tracing::trace;

#[derive(Debug)]
pub enum ScenarioSelection {
    All,
    One(String),
    InRun(String),
    InRange { from: i64, to: i64 },
    Search(String),
}

#[derive(Debug)]
pub enum RunSelection {
    All,
    InRange { from: i64, to: i64 },
    LastN(u64),
}

/// # DatasetBuilder
///
/// DatasetBuilder => DatasetRowPager => DatasetRows => DatasetColPager => DatasetBuilderFinal => Dataset
///
/// The DatasetBuilder allows you to construct a Dataset. There is one case that is not allowed. If you have multiple
/// scenarios (rows) you cannot `page` over runs (columns).
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
/// Example: last 3 runs of [add_10_items, add_10_users, checkout]
///  ============================================
/// ||  scenarios   || run_1  | run_2  | run_3  ||
/// ||--------------||--------------------------||
/// || add_10_items || <data> | <data> |        ||
/// || add_10_users ||        | <data> | <data> ||
/// || checkout     || <data> |        | <data> ||
///  ============================================
///

pub struct DatasetBuilder;
impl DatasetBuilder {
    pub fn new() -> Self {
        DatasetBuilder
    }

    /// Returns a single scenario.
    pub fn scenario(&self, scenario: &str) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::One(scenario.to_string()),
        }
    }

    /// Returns all scenarios.
    pub fn scenarios_all(&self) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::All,
        }
    }

    /// Returns all scenarios that were executed in a single run.
    pub fn scenarios_in_run(&self, run: i32) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::InRun(run.to_string()),
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
        }
    }

    /// Returns a DatasetRowPager all scenarios that match the given name. This function does not fetch these
    /// scenarios, it just defines the maximum set of scenarios which can be filtered in subsequent
    /// steps.
    pub fn scenarios_by_name(&self, name: &str) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::Search(name.to_string()),
        }
    }
}

/// The DatasetRowPager defines an incomplete Dataset which includes set of scenarios (rows)
/// without any runs.
///
/// It provides functions to select a subset within that range of scenarios.
pub struct DatasetRowPager {
    scenario_selection: ScenarioSelection,
}
impl DatasetRowPager {
    /// Returns a DatasetRows object which defined the full set of scenarios defined by this
    /// DatasetRowPager.
    pub fn all(self) -> DatasetRows {
        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: None,
        }
    }

    /// Returns a DatasetRows object which defines a subset of the scenarios defined by this
    /// DatasetRowPager.
    pub fn page(self, page_size: u64, page_num: u64) -> DatasetRows {
        let scenario_page = Page {
            size: page_size,
            num: page_num,
        };

        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: Some(scenario_page),
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
pub struct DatasetRows {
    scenario_selection: ScenarioSelection,
    scenario_page: Option<Page>,
}
impl DatasetRows {
    /// Return a DataColPager which includes all the runs for this scenario.
    pub fn runs_all(self) -> DatasetColPager {
        DatasetColPager {
            scenario_selection: self.scenario_selection,
            scenario_page: self.scenario_page,
            run_selection: RunSelection::All,
        }
    }

    /// Return a DatasetColPager which includes only those runs which were executed within the
    /// given time range.
    ///
    /// * Arguments
    /// - from: unix timestamp in millis
    /// - to: unix timestamp in millis
    pub fn runs_in_range(self, from: i64, to: i64) -> DatasetColPager {
        DatasetColPager {
            scenario_selection: self.scenario_selection,
            scenario_page: self.scenario_page,
            run_selection: RunSelection::InRange { from, to },
        }
    }

    /// Returns a DatasetColPager which includes only the last `n` runs of the given scenario.
    ///
    /// * Arguments
    /// - n: number of runs to include.
    pub fn last_n_runs(self, n: u64) -> DatasetColPager {
        DatasetColPager {
            scenario_selection: self.scenario_selection,
            scenario_page: self.scenario_page,
            run_selection: RunSelection::LastN(n),
        }
    }
}

/// The DatasetColPager defines an incomplete Dataset which includes a single scenario (row) and
/// range of runs for that scenario.
///
/// It provides a single function to select a single page within that range of runs.
#[derive(Debug)]
pub struct DatasetColPager {
    scenario_selection: ScenarioSelection,
    scenario_page: Option<Page>,
    run_selection: RunSelection,
}
impl DatasetColPager {
    pub fn all(self) -> DatasetBuilderFinal {
        DatasetBuilderFinal {
            scenario_selection: self.scenario_selection,
            scenario_page: self.scenario_page,
            run_selection: self.run_selection,
            run_page: None,
        }
    }

    pub fn page(self, page_size: u64, page_num: u64) -> anyhow::Result<DatasetBuilderFinal> {
        trace!("page_size = {}", page_size);
        match self.scenario_selection {
            ScenarioSelection::One(_) => Ok(DatasetBuilderFinal {
                scenario_selection: self.scenario_selection,
                scenario_page: self.scenario_page,
                run_selection: self.run_selection,
                run_page: Some(Page {
                    size: page_size,
                    num: page_num,
                }),
            }),

            _ => Err(anyhow::anyhow!(
                "Unable to paginate over runs if multiple scenarios are selected."
            )),
        }
    }
}

pub struct DatasetBuilderFinal {
    scenario_selection: ScenarioSelection,
    scenario_page: Option<Page>,
    run_selection: RunSelection,
    run_page: Option<Page>,
}
impl DatasetBuilderFinal {
    async fn fetch_scenarios(
        &self,
        db: &DatabaseConnection,
    ) -> anyhow::Result<(Vec<String>, Pages)> {
        let (scenario_names, scenario_pages) = match &self.scenario_selection {
            ScenarioSelection::All => dao::scenario::fetch_all(&self.scenario_page, db).await,
            ScenarioSelection::One(name) => {
                let scenario_name = dao::scenario::fetch(&name, db)
                    .await?
                    .context(format!("Error finding scenario with name {}", name))?;

                Ok((vec![scenario_name], Pages::NotRequired)) // if you only have one scenario then
                                                              // pages are not required!
            }
            ScenarioSelection::Search(name) => {
                dao::scenario::fetch_by_query(&name, &self.scenario_page, db).await
            }
            ScenarioSelection::InRun(run) => {
                dao::scenario::fetch_in_run(&run, &self.scenario_page, db).await
            }
            ScenarioSelection::InRange { from, to } => {
                dao::scenario::fetch_in_range(*from, *to, &self.scenario_page, db).await
            }
        }?;

        let scenario_names = scenario_names
            .iter()
            .map(|s| s.scenario_name.clone())
            .collect::<Vec<_>>();

        Ok((scenario_names, scenario_pages))
    }

    async fn all(&self, db: &DatabaseConnection) -> anyhow::Result<Dataset> {
        let (scenarios, total_scenarios) = self.fetch_scenarios(db).await?;

        let (iterations, total_runs) = match self.run_selection {
            RunSelection::All => {
                let poop = dao::iteration::fetch_runs_all(&scenarios, None, db).await;
                // println!("\n {:?}", poop);
                poop
            }

            RunSelection::InRange { from, to } => {
                dao::iteration::fetch_runs_in_range(&scenarios, from, to, None, db).await
            }

            RunSelection::LastN(n) => {
                dao::iteration::fetch_runs_last_n(&scenarios, n, None, db).await
            }
        }?;

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics =
                dao::metrics::fetch_within(it.run_id, it.start_time, it.stop_time, db).await?;
            iterations_with_metrics.push(IterationMetrics::new(it, metrics));
        }
        // println!("\n {:?}", iterations_with_metrics);

        // TODO: cache the iterations/metrics data
        //

        Ok(Dataset::new(
            iterations_with_metrics,
            total_scenarios,
            total_runs,
        ))
    }

    async fn page(&self, page: &Page, db: &DatabaseConnection) -> anyhow::Result<Dataset> {
        let page = Page::new(page.size, page.num);
        let (scenarios, total_scenarios) = self.fetch_scenarios(db).await?;

        let (iterations, total_runs) = match self.run_selection {
            RunSelection::All => dao::iteration::fetch_runs_all(&scenarios, Some(page), db).await,

            RunSelection::InRange { from, to } => {
                dao::iteration::fetch_runs_in_range(&scenarios, from, to, Some(page), db).await
            }

            RunSelection::LastN(n) => {
                dao::iteration::fetch_runs_last_n(&scenarios, n, Some(page), db).await
            }
        }?;

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics =
                dao::metrics::fetch_within(it.run_id, it.start_time, it.stop_time, db).await?;
            iterations_with_metrics.push(IterationMetrics::new(it, metrics));
        }

        // TODO: cache the iterations/metrics data
        //

        Ok(Dataset::new(
            iterations_with_metrics,
            total_scenarios,
            total_runs,
        ))
    }

    pub async fn build(&self, db: &DatabaseConnection) -> anyhow::Result<Dataset> {
        match &self.run_page {
            Some(page) => self.page(page, db).await,
            None => self.all(db).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        data::{dataset::LiveDataFilter, dataset_builder::DatasetBuilder},
        db_connect, db_migrate,
        tests::setup_fixtures,
    };
    use itertools::Itertools;
    use sea_orm::DatabaseConnection;

    async fn init_tests() -> anyhow::Result<DatabaseConnection> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
            ],
            &db,
        )
        .await?;

        Ok(db)
    }

    #[tokio::test]
    async fn scenarios_all() -> anyhow::Result<()> {
        let db = init_tests().await?;
        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be 3 scenarios in reverse chronological order
        // i.e. [scenario_3, scenario_2, scenario_1]
        let scenario_names = scenario_datasets
            .iter()
            .map(|dataset| dataset.scenario_name())
            .collect_vec();
        assert_eq!(
            scenario_names,
            vec!["scenario_3", "scenario_2", "scenario_1"]
        );

        Ok(())
    }

    #[tokio::test]
    async fn scenario() -> anyhow::Result<()> {
        let db = init_tests().await?;
        let dataset = DatasetBuilder::new()
            .scenario("scenario_2")
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be one scenario (the one we selected) in reverse chronological order
        // ie. [scenario_2]
        let scenario_names = scenario_datasets
            .iter()
            .map(|dataset| dataset.scenario_name())
            .collect_vec();
        assert_eq!(scenario_names, vec!["scenario_2"]);

        Ok(())
    }

    #[tokio::test]
    async fn scenarios_in_run() -> anyhow::Result<()> {
        let db = init_tests().await?;
        let dataset = DatasetBuilder::new()
            .scenarios_in_run(2)
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be two scenarios (the ones in run 2) in reverse chronological order
        // ie. [scenario_3, scenario_2]
        let scenario_names = scenario_datasets
            .iter()
            .map(|dataset| dataset.scenario_name())
            .collect_vec();
        assert_eq!(scenario_names, vec!["scenario_3", "scenario_2"]);

        Ok(())
    }

    #[tokio::test]
    async fn scenarios_in_range() -> anyhow::Result<()> {
        let db = init_tests().await?;
        let dataset = DatasetBuilder::new()
            .scenarios_in_range(1717507690000, 1717507699000)
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be two scenarios (the ones in the given time range) in reverse
        // chronological order
        // ie. [scenario_3, scenario_2]
        let scenario_names = scenario_datasets
            .iter()
            .map(|dataset| dataset.scenario_name())
            .collect_vec();
        assert_eq!(scenario_names, vec!["scenario_3", "scenario_2"]);

        Ok(())
    }

    #[tokio::test]
    async fn scenarios_search() -> anyhow::Result<()> {
        let db = init_tests().await?;
        let dataset = DatasetBuilder::new()
            .scenarios_by_name("scenario")
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be three scenarios (all matching "scenario") in reverse chronological order
        // ie. [scenario_3, scenario_2, scenario_1]
        let scenario_names = scenario_datasets
            .iter()
            .map(|dataset| dataset.scenario_name())
            .collect_vec();
        assert_eq!(
            scenario_names,
            vec!["scenario_3", "scenario_2", "scenario_1"]
        );

        Ok(())
    }

    #[tokio::test]
    async fn runs_all() -> anyhow::Result<()> {
        let db = init_tests().await?;

        // single scenario
        let dataset = DatasetBuilder::new()
            .scenario("scenario_3")
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        let run_datasets = scenario_datasets.get(0).unwrap().by_run();

        // there should be three runs for scenario 3 returned in reverse chronological order
        // ie. [3, 2, 1]
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(run_ids, vec![3, 2, 1]);

        // multiple scenarios
        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .runs_all()
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be 3 runs for scenario_3, 2 for scenario_2 and 1 for scenario_1 in reverse
        // chronological order
        // ie. scenario_3 = [3,2,1], scenario_2 = [2,1], scenario_1 = [1]
        let scenario_dataset = scenario_datasets.get(0).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_3");
        assert_eq!(run_ids, vec![3, 2, 1]);

        let scenario_dataset = scenario_datasets.get(1).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_2");
        assert_eq!(run_ids, vec![2, 1]);

        let scenario_dataset = scenario_datasets.get(2).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_1");
        assert_eq!(run_ids, vec![1]);

        Ok(())
    }

    #[tokio::test]
    async fn runs_in_range() -> anyhow::Result<()> {
        let db = init_tests().await?;

        // single scenario
        let dataset = DatasetBuilder::new()
            .scenario("scenario_3")
            .all()
            .runs_in_range(1717507690000, 1717507795000)
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        let run_datasets = scenario_datasets.get(0).unwrap().by_run();

        // there should be 2 runs for scenario 3 returned in reverse chronological order
        // ie. [3, 2]
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(run_ids, vec![3, 2]);

        // multiple scenarios
        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .runs_in_range(1717507690000, 1717507795000)
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be 2 runs for scenario_3 and 1 for scenario_2 in reverse chronological order
        // ie. scenario_3 = [3,2], scenario_2 = [2]
        let scenario_dataset = scenario_datasets.get(0).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_3");
        assert_eq!(run_ids, vec![3, 2]);

        let scenario_dataset = scenario_datasets.get(1).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_2");
        assert_eq!(run_ids, vec![2]);

        Ok(())
    }

    #[tokio::test]
    async fn runs_last_n() -> anyhow::Result<()> {
        let db = init_tests().await?;

        // single scenario
        let dataset = DatasetBuilder::new()
            .scenario("scenario_3")
            .all()
            .last_n_runs(2)
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        let run_datasets = scenario_datasets.get(0).unwrap().by_run();

        // there should be 2 runs for scenario 3 returned in reverse chronological order
        // ie. [3, 2]
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(run_ids, vec![3, 2]);

        // multiple scenarios
        let dataset = DatasetBuilder::new()
            .scenarios_all()
            .all()
            .last_n_runs(2)
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);

        // there should be 2 runs for scenario_3, 2 for scenario_2 and 1 for scenario_1 in reverse chronological order
        // ie. scenario_3 = [3,2], scenario_2 = [2,1], scenario_1 = [1]
        let scenario_dataset = scenario_datasets.get(0).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_3");
        assert_eq!(run_ids, vec![3, 2]);

        let scenario_dataset = scenario_datasets.get(1).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_2");
        assert_eq!(run_ids, vec![2, 1]);

        let scenario_dataset = scenario_datasets.get(2).unwrap();
        let run_datasets = scenario_dataset.by_run();
        let run_ids = run_datasets
            .iter()
            .map(|dataset| dataset.run_id())
            .collect_vec();
        assert_eq!(scenario_dataset.scenario_name(), "scenario_1");
        assert_eq!(run_ids, vec![1]);

        Ok(())
    }

    #[tokio::test]
    async fn iterations() -> anyhow::Result<()> {
        let db = init_tests().await?;

        // single scenario
        let dataset = DatasetBuilder::new()
            .scenario("scenario_3")
            .all()
            .last_n_runs(1)
            .all()
            .build(&db)
            .await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        let run_datasets = scenario_datasets.get(0).unwrap().by_run();
        let run_dataset = run_datasets.get(0).unwrap();

        // there should be three runs for scenario 3 returned in reverse chronological order
        // ie. [3]
        let iteration_metrics = run_dataset.by_iteration();
        assert_eq!(iteration_metrics.len(), 3);

        Ok(())
    }
}
