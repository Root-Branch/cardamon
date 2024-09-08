use crate::{
    dao::{self, pagination::Page},
    data::dataset::{Dataset, IterationMetrics},
};
use sea_orm::DatabaseConnection;

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
    db: &'a DatabaseConnection,
}
impl<'a> DatasetBuilder<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        DatasetBuilder { db }
    }

    /// Returns a single scenario.
    pub fn scenario(&self, scenario: &str) -> DatasetRow {
        DatasetRow {
            scenario: scenario.to_string(),
            db: self.db,
        }
    }

    /// Returns all scenarios.
    pub fn scenarios_all(&self) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::All,
            db: self.db,
        }
    }

    /// Returns all scenarios that were executed in a single run.
    pub fn scenarios_in_run(&self, run: i32) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::InRun(run.to_string()),
            db: self.db,
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
            db: self.db,
        }
    }

    /// Returns a DatasetRowPager all scenarios that match the given name. This function does not fetch these
    /// scenarios, it just defines the maximum set of scenarios which can be filtered in subsequent
    /// steps.
    pub fn scenarios_by_name(&self, name: &str) -> DatasetRowPager {
        DatasetRowPager {
            scenario_selection: ScenarioSelection::Search(name.to_string()),
            db: self.db,
        }
    }
}

/// The DatasetRowPager defines an incomplete Dataset which includes set of scenarios (rows)
/// without any runs.
///
/// It provides functions to select a subset within that range of scenarios.
pub struct DatasetRowPager<'a> {
    scenario_selection: ScenarioSelection,
    db: &'a DatabaseConnection,
}
impl<'a> DatasetRowPager<'a> {
    /// Returns a DatasetRows object which defined the full set of scenarios defined by this
    /// DatasetRowPager.
    pub fn all(self) -> DatasetRows<'a> {
        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: None,
            db: self.db,
        }
    }

    /// Returns a DatasetRows object which defines a subset of the scenarios defined by this
    /// DatasetRowPager.
    pub fn page(self, page_size: u64, page_num: u64) -> DatasetRows<'a> {
        let scenario_page = Page {
            size: page_size,
            num: page_num,
        };

        DatasetRows {
            scenario_selection: self.scenario_selection,
            scenario_page: Some(scenario_page),
            db: self.db,
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
    db: &'a DatabaseConnection,
}
impl<'a> DatasetRows<'a> {
    /// Returns a Dataset which contains the iterations and metrics collected in the last 'n' runs
    /// of each scenario.
    ///
    /// This function is async as it uses the dao_service to fetch the results from the db.
    pub async fn last_n_runs(&self, n: u64) -> anyhow::Result<Dataset> {
        let (scenarios, total_scenarios) = match &self.scenario_selection {
            ScenarioSelection::All => dao::scenario::fetch_all(&self.scenario_page, self.db).await,
            ScenarioSelection::Search(name) => {
                dao::scenario::fetch_by_name(name, &self.scenario_page, self.db).await
            }
            ScenarioSelection::InRun(run) => {
                dao::scenario::fetch_in_run(run, &self.scenario_page, self.db).await
            }
            ScenarioSelection::InRange { from, to } => {
                dao::scenario::fetch_in_range(*from, *to, &self.scenario_page, self.db).await
            }
        }?;

        // for each scenario get the associated iterations in the last n runs
        let mut iterations = vec![];
        for scenario in scenarios {
            let mut scenario_iterations =
                dao::iteration::fetch_runs_last_n(&scenario.scenario_name, n, self.db).await?;
            iterations.append(&mut scenario_iterations);
        }

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics =
                dao::metrics::fetch_within(it.run_id, it.start_time, it.stop_time, self.db).await?;
            iterations_with_metrics.push(IterationMetrics::new(it, metrics));
        }

        // TODO: cache the iterations/metrics data

        Ok(Dataset::new(iterations_with_metrics, total_scenarios))
    }
}

/// The DatasetRow defines an incomplete Dataset with a single scenario (row) without any runs.
/// This object provides functions for defining a range of runs to include for the scenario.
pub struct DatasetRow<'a> {
    scenario: String,
    db: &'a DatabaseConnection,
}
impl<'a> DatasetRow<'a> {
    /// Return a DataColPager which includes all the runs for this scenario.
    pub fn runs_all(self) -> DatasetColPager<'a> {
        DatasetColPager {
            scenario: self.scenario,
            run_selection: RunSelection::All,
            db: self.db,
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
            db: self.db,
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
    db: &'a DatabaseConnection,
}
impl<'a> DatasetColPager<'a> {
    pub async fn page(&self, page_size: u64, page_num: u64) -> anyhow::Result<Dataset> {
        let page = Page::new(page_size, page_num);

        let (iterations, _total_runs) = match self.run_selection {
            RunSelection::All => {
                dao::iteration::fetch_runs_all(&self.scenario, &Some(page), self.db).await
            }

            RunSelection::InRange { from, to } => {
                dao::iteration::fetch_runs_in_range(&self.scenario, from, to, &Some(page), self.db)
                    .await
            }
        }?;

        // marry up iterations with metrics
        // TODO: read from cache table first
        let mut iterations_with_metrics = vec![];
        for it in iterations {
            let metrics =
                dao::metrics::fetch_within(it.run_id, it.start_time, it.stop_time, self.db).await?;
            iterations_with_metrics.push(IterationMetrics::new(it, metrics));
        }

        // TODO: cache the iterations/metrics data
        //

        Ok(Dataset::new(iterations_with_metrics, 1))
    }
}
