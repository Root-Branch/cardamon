use std::fs;
use std::path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use anyhow::Context;

use nanoid::nanoid;

#[derive(Debug, sqlx::FromRow, PartialEq)]
pub struct Scenario {
    pub id: String,
    pub cardamon_run_id: String,
    pub scenario_name: String,
    pub start_time: i64,
    pub stop_time: i64,
}

impl Scenario {
    pub fn new(
        cardamon_run_id: &str,
        scenario_name: &str,
        start_time: i64,
        stop_time: i64,
    ) -> Self {
        Scenario {
            id: nanoid!(5),
            cardamon_run_id: String::from(cardamon_run_id),
            scenario_name: String::from(scenario_name),
            start_time,
            stop_time,
        }
    }

    pub async fn fetch(id: &str, pool: &sqlx::SqlitePool) -> anyhow::Result<Option<Self>> {
        sqlx::query_as!(Scenario, "SELECT * FROM scenario WHERE id = ?1", id)
            .fetch_optional(pool)
            .await
            .context("Error fetching scenario with id {id}")
    }

    pub async fn persist(&self, pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO scenario (id, cardamon_run_id, scenario_name, start_time, stop_time) VALUES (?1, ?2, ?3, ?4, ?5)", 
            self.id,
            self.cardamon_run_id,
            self.scenario_name,
            self.start_time,
            self.stop_time)
            .execute(pool)
            .await
            .map(|_| ())
            .context("Error inserting scenario into db.")
    }

    pub async fn delete(id: &str, pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM scenario WHERE id = ?1", id)
            .execute(pool)
            .await
            .map(|_| ())
            .context("Error deleting scenario with id {id}")
    }
}

#[derive(Debug, sqlx::FromRow, PartialEq)]
pub struct CpuMetrics {
    pub id: String,
    pub cardamon_run_id: String,
    pub process_id: String,
    pub process_name: String,
    pub cpu_usage: f64,
    pub total_usage: f64,
    pub core_count: i64,
    pub timestamp: i64,
}

impl CpuMetrics {
    pub fn new(
        cardamon_run_id: &str,
        process_id: &str,
        process_name: &str,
        cpu_usage: f64,
        total_usage: f64,
        core_count: i64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("UNIX_EPOCH should be before now!")
            .as_millis() as i64; // we can worry about this conversion in 1000 years time!

        CpuMetrics {
            id: nanoid!(5),
            cardamon_run_id: String::from(cardamon_run_id),
            process_id: String::from(process_id),
            process_name: String::from(process_name),
            cpu_usage,
            total_usage,
            core_count,
            timestamp,
        }
    }

    pub async fn fetch(id: &str, pool: &sqlx::SqlitePool) -> anyhow::Result<Option<Self>> {
        sqlx::query_as!(CpuMetrics, "SELECT * FROM cpu_metrics WHERE id = ?1", id)
            .fetch_optional(pool)
            .await
            .context("Error fetching cpu metrics from db.")
    }

    pub async fn persist(&self, pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO cpu_metrics (id, cardamon_run_id, process_id, process_name, cpu_usage, total_usage, core_count, timestamp) \
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", 
            self.id,
            self.cardamon_run_id,
            self.process_id,
            self.process_name,
            self.cpu_usage,
            self.total_usage,
            self.core_count,
            self.timestamp
        )
            .execute(pool)
            .await
            .map(|_| ())
            .context("Error inserting cpu metrics into db.")
    }

    pub async fn delete(id: &str, pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM cpu_metrics WHERE id = ?1", id)
            .execute(pool)
            .await
            .map(|_| ())
            .context("Error deleting cpu metrics with id {id}")
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
    use core::panic;

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

    #[sqlx::test(migrations = "./migrations")]
    async fn test_cpu_metrics_crd(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let metrics = CpuMetrics::new("1", "1", "test_process", 200_f64, 100_f64, 4);
        metrics.persist(&pool).await?;

        match CpuMetrics::fetch(&metrics.id, &pool).await? {
            Some(fetched) => assert_eq!(fetched, metrics),
            None => panic!("metrics not found!"),
        }

        CpuMetrics::delete(&metrics.id, &pool).await?;

        if CpuMetrics::fetch(&metrics.id, &pool).await?.is_some() {
            panic!("metrics should not exist after delete!");
        }

        pool.close().await;
        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_scenarios_crd(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

        let scenario = Scenario::new("1", "1", timestamp, timestamp + 10000);
        scenario.persist(&pool).await?;

        match Scenario::fetch(&scenario.id, &pool).await? {
            Some(fetched) => assert_eq!(fetched, scenario),
            None => panic!("scenario not found!"),
        }

        Scenario::delete(&scenario.id, &pool).await?;

        if Scenario::fetch(&scenario.id, &pool).await?.is_some() {
            panic!("scenario should not exist after delete!");
        }

        pool.close().await;
        Ok(())
    }
}
