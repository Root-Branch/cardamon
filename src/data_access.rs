/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub mod cpu_metrics;
pub mod scenario_iteration;

use crate::dataset::{IterationWithMetrics, ObservationDataset};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use cpu_metrics::CpuMetricsDao;
use scenario_iteration::ScenarioIterationDao;
use sqlx::SqlitePool;
use std::{fs, path};

#[async_trait]
pub trait DataAccessService: Send + Sync {
    fn scenario_iteration_dao(&self) -> &dyn ScenarioIterationDao;
    fn cpu_metrics_dao(&self) -> &dyn CpuMetricsDao;

    async fn fetch_observation_dataset(
        &self,
        scenario_names: Vec<&str>,
        previous_runs: u32,
    ) -> anyhow::Result<ObservationDataset> {
        // for each scenario, get the last `n` runs (including all iterations)
        // grab the metrics associated with with run and group the data by scenario name.
        let mut all_scenario_iterations_with_metrics = vec![];
        for scenario_name in scenario_names.iter() {
            let scenario_iterations = self
                .scenario_iteration_dao()
                .fetch_last(scenario_name, previous_runs)
                .await?;

            let mut scenario_iterations_with_metrics = vec![];
            for scenario_iteration in scenario_iterations.into_iter() {
                let cpu_metrics = self
                    .cpu_metrics_dao()
                    .fetch_within(
                        &scenario_iteration.run_id,
                        scenario_iteration.start_time,
                        scenario_iteration.stop_time,
                    )
                    .await?;

                let scenario_iteration_with_metrics =
                    IterationWithMetrics::new(scenario_iteration, cpu_metrics);

                scenario_iterations_with_metrics.push(scenario_iteration_with_metrics);
            }
            all_scenario_iterations_with_metrics.append(&mut scenario_iterations_with_metrics);
        }
        all_scenario_iterations_with_metrics.reverse();

        Ok(ObservationDataset::new(
            all_scenario_iterations_with_metrics,
        ))
    }
}

pub struct LocalDataAccessService {
    scenario_iteration_dao: scenario_iteration::LocalDao,
    cpu_metrics_dao: cpu_metrics::LocalDao,
}
impl LocalDataAccessService {
    pub fn new(pool: SqlitePool) -> Self {
        let scenario_iteration_dao = scenario_iteration::LocalDao::new(pool.clone());
        let cpu_metrics_dao = cpu_metrics::LocalDao::new(pool.clone());

        Self {
            scenario_iteration_dao,
            cpu_metrics_dao,
        }
    }
}
impl DataAccessService for LocalDataAccessService {
    fn scenario_iteration_dao(&self) -> &dyn ScenarioIterationDao {
        &self.scenario_iteration_dao
    }

    fn cpu_metrics_dao(&self) -> &dyn CpuMetricsDao {
        &self.cpu_metrics_dao
    }
}

pub struct RemoteDataAccessService {
    scenario_iteration_dao: scenario_iteration::RemoteDao,
    cpu_metrics_dao: cpu_metrics::RemoteDao,
}
impl RemoteDataAccessService {
    pub fn new(base_url: &str) -> Self {
        let scenario_iteration_dao = scenario_iteration::RemoteDao::new(base_url);
        let cpu_metrics_dao = cpu_metrics::RemoteDao::new(base_url);

        Self {
            scenario_iteration_dao,
            cpu_metrics_dao,
        }
    }
}
impl DataAccessService for RemoteDataAccessService {
    fn scenario_iteration_dao(&self) -> &dyn ScenarioIterationDao {
        &self.scenario_iteration_dao
    }

    fn cpu_metrics_dao(&self) -> &dyn CpuMetricsDao {
        &self.cpu_metrics_dao
    }
}

pub async fn connect(conn_str: &str) -> anyhow::Result<sqlx::SqlitePool> {
    let conn_str = conn_str.trim();

    // break string into database type and database uri
    let (db_type, db_uri) = conn_str.split_once(':').ok_or(anyhow!("Unable to split connection string into database type and uri. Is the connection string formated correctly?"))?;

    // if trying to connect to an sqlite database, make sure the
    // database file exists
    if db_type == "sqlite" && db_uri != ":memory:" {
        // strip '//' from database path
        let db_uri = db_uri.replacen("//", "", 1);

        // if the path doesn't exist then attempt to create it
        if !path::Path::new(&db_uri).exists() {
            fs::File::create(db_uri).context("unable to create sqlite database file.")?;
        }
    }

    // construct a new AnyPool
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .max_connections(4)
        .connect(conn_str)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection() -> anyhow::Result<()> {
        let pool = connect("sqlite::memory:").await?;

        let (res,): (i64,) = sqlx::query_as("SELECT $1")
            .bind(42_i64)
            .fetch_one(&pool)
            .await?;

        assert_eq!(res, 42);

        pool.close().await;
        Ok(())
    }
}
