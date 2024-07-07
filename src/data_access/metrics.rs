use anyhow::Context;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct Metrics {
    pub run_id: String,
    pub process_id: String,
    pub process_name: String,
    pub cpu_usage: f64,
    pub cpu_total_usage: f64,
    pub cpu_core_count: i64,
    pub time_stamp: i64,
}
impl Metrics {
    pub fn new(
        run_id: &str,
        process_id: &str,
        process_name: &str,
        cpu_usage: f64,
        cpu_total_usage: f64,
        cpu_core_count: i64,
        time_stamp: i64,
    ) -> Self {
        Metrics {
            run_id: String::from(run_id),
            process_id: String::from(process_id),
            process_name: String::from(process_name),
            cpu_usage,
            cpu_total_usage,
            cpu_core_count,
            time_stamp,
        }
    }
}

#[async_trait]
pub trait MetricsDao {
    /// Return the metrics for the given run within the given time range.
    async fn fetch_within(&self, run: &str, from: i64, to: i64) -> anyhow::Result<Vec<Metrics>>;

    /// Persist a metrics object to the db.
    async fn persist(&self, metrics: &Metrics) -> anyhow::Result<()>;
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
impl MetricsDao for LocalDao {
    async fn fetch_within(&self, run: &str, from: i64, to: i64) -> anyhow::Result<Vec<Metrics>> {
        sqlx::query_as!(
            Metrics,
            "SELECT * FROM metrics WHERE run_id = ?1 AND time_stamp >= ?2 AND time_stamp <= ?3",
            run,
            from,
            to
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching cpu metrics from db.")
    }

    async fn persist(&self, metrics: &Metrics) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO metrics (
                run_id, 
                process_id, 
                process_name, 
                cpu_usage, 
                cpu_total_usage, 
                cpu_core_count, 
                time_stamp
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            metrics.run_id,
            metrics.process_id,
            metrics.process_name,
            metrics.cpu_usage,
            metrics.cpu_total_usage,
            metrics.cpu_core_count,
            metrics.time_stamp
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .context("Error inserting cpu metrics into db.")
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
impl MetricsDao for RemoteDao {
    async fn fetch_within(
        &self,
        run_id: &str,
        begin: i64,
        end: i64,
    ) -> anyhow::Result<Vec<Metrics>> {
        self.client
            .get(format!(
                "{}/metrics/{run_id}?begin={begin}&end={end}",
                self.base_url
            ))
            .send()
            .await?
            .json::<Vec<Metrics>>()
            .await
            .context("Error fetching cpu metrics with id {id} from remote server")
    }

    async fn persist(&self, metrics: &Metrics) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/metrics", self.base_url))
            .json(metrics)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting cpu metrics to remote server")
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../fixtures/runs.sql", "../../fixtures/metrics.sql")
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
