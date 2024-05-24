/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use super::DataAccess;
use anyhow::Context;
use nanoid::nanoid;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
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
}

// //////////////////////////////////////
// LocalDao

pub struct LocalDao<'a> {
    pub pool: &'a sqlx::SqlitePool,
}
impl<'a> LocalDao<'a> {
    pub fn new(pool: &'a sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}
impl<'a> DataAccess<CpuMetrics> for LocalDao<'a> {
    async fn fetch(&self, id: &str) -> anyhow::Result<Option<CpuMetrics>> {
        sqlx::query_as!(CpuMetrics, "SELECT * FROM cpu_metrics WHERE id = ?1", id)
            .fetch_optional(self.pool)
            .await
            .context("Error fetching cpu metrics from db.")
    }

    async fn persist(&self, metrics: &CpuMetrics) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO cpu_metrics (id, cardamon_run_id, process_id, process_name, cpu_usage, total_usage, core_count, timestamp) \
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", 
            metrics.id,
            metrics.cardamon_run_id,
            metrics.process_id,
            metrics.process_name,
            metrics.cpu_usage,
            metrics.total_usage,
            metrics.core_count,
            metrics.timestamp
        )
            .execute(self.pool)
            .await
            .map(|_| ())
            .context("Error inserting cpu metrics into db.")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM cpu_metrics WHERE id = ?1", id)
            .execute(self.pool)
            .await
            .map(|_| ())
            .context("Error deleting cpu metrics with id {id}")
    }
}

// //////////////////////////////////////
// RemoteDao

pub struct RemoteDao {
    base_url: String,
    client: reqwest::Client,
}
impl RemoteDao {
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.strip_suffix('/').unwrap_or(base_url);
        Self {
            base_url: String::from(base_url),
            client: reqwest::Client::new(),
        }
    }
}
impl DataAccess<CpuMetrics> for RemoteDao {
    async fn fetch(&self, id: &str) -> anyhow::Result<Option<CpuMetrics>> {
        self.client
            .get(format!("{}/cpu_metrics/{id}", self.base_url))
            .send()
            .await?
            .json::<Option<CpuMetrics>>()
            .await
            .context("Error fetching cpu metrics with id {id} from remote server")
    }

    async fn persist(&self, metrics: &CpuMetrics) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/cpu_metrics", self.base_url))
            .json(metrics)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting cpu metrics to remote server")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.client
            .delete(format!("{}/cpu_metrics/{id}", self.base_url))
            .send()
            .await
            .map(|_| ())
            .context("Error deleting cpu metrics with id {id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::panic;

    #[sqlx::test(migrations = "./migrations")]
    async fn test_local_cpu_metrics_service(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let metrics_service = LocalDao::new(&pool);

        let metrics = CpuMetrics::new("1", "1", "test_process", 200_f64, 100_f64, 4);
        metrics_service.persist(&metrics).await?;

        match metrics_service.fetch(&metrics.id).await? {
            Some(fetched) => assert_eq!(fetched, metrics),
            None => panic!("metrics not found!"),
        }

        metrics_service.delete(&metrics.id).await?;

        if metrics_service.fetch(&metrics.id).await?.is_some() {
            panic!("metrics should not exist after delete!");
        }

        pool.close().await;
        Ok(())
    }
}
