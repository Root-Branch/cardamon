/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub mod cpu_metrics;
pub mod scenario_run;

use anyhow::{anyhow, Context};
use cpu_metrics::CpuMetrics;
use scenario_run::ScenarioRun;
use sqlx::SqlitePool;
use std::rc::Rc;
use std::{fs, future::Future, path};

pub trait DataAccess<T> {
    fn fetch(&self, id: &str) -> impl Future<Output = anyhow::Result<Option<T>>> + Send;
    fn persist(&self, model: &T) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = anyhow::Result<()>> + Send;
}

pub trait DataAccessService {
    fn scenario_run_dao(&self) -> Rc<impl DataAccess<ScenarioRun>>;
    fn cpu_metrics_dao(&self) -> Rc<impl DataAccess<CpuMetrics>>;
}

pub struct LocalDataAccessService {
    scenario_run_dao: Rc<scenario_run::LocalDao>,
    cpu_metrics_dao: Rc<cpu_metrics::LocalDao>,
}
impl LocalDataAccessService {
    pub fn new(pool: SqlitePool) -> Self {
        let scenario_run_dao = Rc::new(scenario_run::LocalDao::new(pool.clone()));
        let cpu_metrics_dao = Rc::new(cpu_metrics::LocalDao::new(pool.clone()));

        Self {
            scenario_run_dao,
            cpu_metrics_dao,
        }
    }
}
impl DataAccessService for LocalDataAccessService {
    fn scenario_run_dao(&self) -> Rc<impl DataAccess<ScenarioRun>> {
        self.scenario_run_dao.clone()
    }

    fn cpu_metrics_dao(&self) -> Rc<impl DataAccess<CpuMetrics>> {
        self.cpu_metrics_dao.clone()
    }
}

pub struct RemoteDataAccessService {
    scenario_run_dao: Rc<scenario_run::RemoteDao>,
    cpu_metrics_dao: Rc<cpu_metrics::RemoteDao>,
}
impl RemoteDataAccessService {
    pub fn new(base_url: &str) -> Self {
        let scenario_run_dao = Rc::new(scenario_run::RemoteDao::new(base_url));
        let cpu_metrics_dao = Rc::new(cpu_metrics::RemoteDao::new(base_url));

        Self {
            scenario_run_dao,
            cpu_metrics_dao,
        }
    }
}
impl DataAccessService for RemoteDataAccessService {
    fn scenario_run_dao(&self) -> Rc<impl DataAccess<ScenarioRun>> {
        self.scenario_run_dao.clone()
    }

    fn cpu_metrics_dao(&self) -> Rc<impl DataAccess<CpuMetrics>> {
        self.cpu_metrics_dao.clone()
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
