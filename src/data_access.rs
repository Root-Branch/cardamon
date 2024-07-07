pub mod iteration;
pub mod metrics;
pub mod pagination;
pub mod scenario;

use self::scenario::ScenarioDao;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use iteration::IterationDao;
use metrics::MetricsDao;
use sqlx::SqlitePool;
use std::{fs, path};

#[async_trait]
pub trait DAOService: Send + Sync {
    fn scenarios(&self) -> &dyn ScenarioDao;
    fn iterations(&self) -> &dyn IterationDao;
    fn metrics(&self) -> &dyn MetricsDao;
}

pub struct LocalDataAccessService {
    scenarios: scenario::LocalDao,
    iterations: iteration::LocalDao,
    metrics: metrics::LocalDao,
}
impl LocalDataAccessService {
    pub fn new(pool: SqlitePool) -> Self {
        let scenarios = scenario::LocalDao::new(pool.clone());
        let iterations = iteration::LocalDao::new(pool.clone());
        let metrics = metrics::LocalDao::new(pool.clone());

        Self {
            scenarios,
            iterations,
            metrics,
        }
    }
}
impl DAOService for LocalDataAccessService {
    fn scenarios(&self) -> &dyn ScenarioDao {
        &self.scenarios
    }

    fn iterations(&self) -> &dyn IterationDao {
        &self.iterations
    }

    fn metrics(&self) -> &dyn MetricsDao {
        &self.metrics
    }
}

pub struct RemoteDataAccessService {
    _scenarios: scenario::RemoteDao,
    _iterations: iteration::RemoteDao,
    _metrics: metrics::RemoteDao,
}
impl RemoteDataAccessService {
    pub fn new(base_url: &str) -> Self {
        let scenarios = scenario::RemoteDao::new(base_url);
        let iterations = iteration::RemoteDao::new(base_url);
        let metrics = metrics::RemoteDao::new(base_url);

        Self {
            _scenarios: scenarios,
            _iterations: iterations,
            _metrics: metrics,
        }
    }
}
impl DAOService for RemoteDataAccessService {
    fn scenarios(&self) -> &dyn ScenarioDao {
        &self._scenarios
    }

    fn iterations(&self) -> &dyn IterationDao {
        &self._iterations
    }

    fn metrics(&self) -> &dyn MetricsDao {
        &self._metrics
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
