use crate::{
    dao::{
        self,
        pagination::{Page, Pages},
    },
    data::dataset::{Dataset, IterationMetrics},
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

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
            RunSelection::All => dao::iteration::fetch_runs_all(&scenarios, None, db).await,

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
