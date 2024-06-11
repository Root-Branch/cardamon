/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Context;
use async_trait::async_trait;
use nanoid::nanoid;

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
        timestamp: i64,
    ) -> Self {
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

#[async_trait]
pub trait CpuMetricsDao {
    async fn fetch_within(
        &self,
        cardamon_run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<CpuMetrics>>;
    async fn persist(&self, model: &CpuMetrics) -> anyhow::Result<()>;
    async fn delete(&self, id: &str) -> anyhow::Result<()>;
}

// //////////////////////////////////////
// LocalDao

pub struct LocalDao {
    pub pool: sqlx::SqlitePool,
}
impl LocalDao {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl CpuMetricsDao for LocalDao {
    async fn fetch_within(
        &self,
        cardamon_run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<CpuMetrics>> {
        sqlx::query_as!(CpuMetrics, r#"
            SELECT * FROM cpu_metrics WHERE cardamon_run_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
            "#, cardamon_run_id, begin, end)
            .fetch_all(&self.pool)
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
            .execute(&self.pool)
            .await
            .map(|_| ())
            .context("Error inserting cpu metrics into db.")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM cpu_metrics WHERE id = ?1", id)
            .execute(&self.pool)
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
#[async_trait]
impl CpuMetricsDao for RemoteDao {
    async fn fetch_within(
        &self,
        cardamon_run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<CpuMetrics>> {
        self.client
            .get(format!(
                "{}/cpu_metrics/{cardamon_run_id}?begin={begin}&end={end}",
                self.base_url
            ))
            .send()
            .await?
            .json::<Vec<CpuMetrics>>()
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
    use itertools::Itertools;

    use super::*;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../fixtures/cpu_metrics.sql")
    )]
    async fn local_cpu_metrics_fetch_within(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let metrics_service = LocalDao::new(pool.clone());

        let metrics = metrics_service
            .fetch_within("1", 1717507600000, 1717507600200)
            .await?;

        assert_eq!(metrics.len(), 4);

        let process_names: Vec<&str> = metrics
            .iter()
            .map(|metric| metric.process_name.as_str())
            .unique()
            .collect();

        assert_eq!(process_names, vec!["yarn", "docker"]);

        pool.close().await;
        Ok(())
    }
    /*
    #[sqlx::test(migrations = "./migrations")]
    async fn test_remote_cpu_metrics_service(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let metrics_service = RemoteDao::new("http://127.0.0.1:4001");

        let metrics = CpuMetrics::new("1", "1", "test_process", 200_f64, 100_f64, 4);
        metrics_service.persist(&metrics).await?;

        match metrics_service.fetch(&metrics.id).await? {
            Some(fetched) => assert_eq!(fetched, metrics),
            None => panic!("metrics not found!"),
        }

        metrics_service.delete(&metrics.id).await?;

        match metrics_service.fetch(&metrics.id).await {
            Ok(m) => panic!("Metrics found after delete {:?}", m),
            Err(_) => (), // Expected none
        }

        pool.close().await;
        Ok(())
    }
    */
}
