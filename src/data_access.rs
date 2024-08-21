pub mod iteration;
pub mod metrics;
pub mod pagination;
pub mod run;
pub mod scenario;

use self::scenario::ScenarioDao;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use iteration::{Iteration, IterationDao};
use metrics::MetricsDao;
use run::RunDao;
use sqlx::SqlitePool;
use std::fmt::Debug;
use std::{fs, path};

#[async_trait]
pub trait DAOService: Send + Sync {
    fn scenarios(&self) -> &dyn ScenarioDao;
    fn iterations(&self) -> &dyn IterationDao;
    fn metrics(&self) -> &dyn MetricsDao;
    fn runs(&self) -> &dyn RunDao;
}

#[derive(Clone, Debug)]
pub struct LocalDAOService {
    scenarios: scenario::LocalDao,
    iterations: iteration::LocalDao,
    metrics: metrics::LocalDao,
    runs: run::LocalDao,
}
impl LocalDAOService {
    pub fn new(pool: SqlitePool) -> Self {
        let scenarios = scenario::LocalDao::new(pool.clone());
        let iterations = iteration::LocalDao::new(pool.clone());
        let metrics = metrics::LocalDao::new(pool.clone());
        let runs = run::LocalDao::new(pool.clone());
        Self {
            scenarios,
            iterations,
            metrics,
            runs,
        }
    }
    pub async fn fetch_unique_run_ids(&self, scenario_name: &str) -> anyhow::Result<Vec<String>> {
        self.iterations.fetch_unique_run_ids(scenario_name).await
    }

    pub async fn fetch_by_scenario_and_run(
        &self,
        scenario_name: &str,
        run_id: &str,
    ) -> anyhow::Result<Vec<Iteration>> {
        self.iterations
            .fetch_by_scenario_and_run(scenario_name, run_id)
            .await
    }
}
impl DAOService for LocalDAOService {
    fn scenarios(&self) -> &dyn ScenarioDao {
        &self.scenarios
    }

    fn iterations(&self) -> &dyn IterationDao {
        &self.iterations
    }

    fn metrics(&self) -> &dyn MetricsDao {
        &self.metrics
    }

    fn runs(&self) -> &dyn RunDao {
        &self.runs
    }
}

pub struct RemoteDAOService {
    scenarios: scenario::RemoteDao,
    iterations: iteration::RemoteDao,
    metrics: metrics::RemoteDao,
    runs: run::RemoteDao,
}
impl RemoteDAOService {
    pub fn new(base_url: &str) -> Self {
        let scenarios = scenario::RemoteDao::new(base_url);
        let iterations = iteration::RemoteDao::new(base_url);
        let metrics = metrics::RemoteDao::new(base_url);
        let runs = run::RemoteDao::new(base_url);

        Self {
            scenarios,
            iterations,
            metrics,
            runs,
        }
    }
}
impl DAOService for RemoteDAOService {
    fn scenarios(&self) -> &dyn ScenarioDao {
        &self.scenarios
    }

    fn iterations(&self) -> &dyn IterationDao {
        &self.iterations
    }

    fn metrics(&self) -> &dyn MetricsDao {
        &self.metrics
    }
    fn runs(&self) -> &dyn RunDao {
        &self.runs
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
