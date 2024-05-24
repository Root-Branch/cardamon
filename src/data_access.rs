pub mod cpu_metrics;
pub mod scenario;

use anyhow::{anyhow, Context};
use sqlx::SqlitePool;
use std::{fs, future::Future, path};

pub trait DataAccess<T> {
    fn fetch(&self, id: &str) -> impl Future<Output = anyhow::Result<Option<T>>> + Send;
    fn persist(&self, model: &T) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = anyhow::Result<()>> + Send;
}

pub enum DataAccessService<'a> {
    Local {
        scenario_dao: scenario::LocalDao<'a>,
        cpu_metrics_dao: cpu_metrics::LocalDao<'a>,
    },

    Remote {
        scenario_dao: scenario::RemoteDao,
        cpu_metrics_dao: cpu_metrics::RemoteDao,
    },
}
impl<'a> DataAccessService<'a> {
    pub fn local(pool: &'a SqlitePool) -> DataAccessService<'a> {
        DataAccessService::Local {
            scenario_dao: scenario::LocalDao::new(pool),
            cpu_metrics_dao: cpu_metrics::LocalDao::new(pool),
        }
    }

    pub fn remote(base_url: &str) -> DataAccessService<'a> {
        DataAccessService::Remote {
            scenario_dao: scenario::RemoteDao::new(base_url),
            cpu_metrics_dao: cpu_metrics::RemoteDao::new(base_url),
        }
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
