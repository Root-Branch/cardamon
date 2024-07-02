use anyhow::Context;
use async_trait::async_trait;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct Run {
    pub id: String,
    pub start_time: i64,
    pub stop_time: Option<i64>,
}
impl Run {
    pub fn new(id: &str, start_time: i64) -> Self {
        Self {
            id: String::from(id),
            start_time,
            stop_time: None,
        }
    }

    pub fn stop(&mut self, stop_time: i64) {
        self.stop_time = Some(stop_time);
    }
}

#[async_trait]
pub trait RunDao {
    async fn fetch(&self, page_size: u32, page: u32) -> anyhow::Result<Vec<Run>>;
    async fn fetch_within(&self, start_time: i64, stop_time: i64) -> anyhow::Result<Vec<Run>>;
    async fn persist(&self, run: &Run) -> anyhow::Result<()>;
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
impl RunDao for LocalDao {
    async fn fetch(&self, page_size: u32, page: u32) -> anyhow::Result<Vec<Run>> {
        let offset = page * page_size;
        sqlx::query_as!(
            Run,
            "SELECT * FROM run ORDER BY start_time DESC LIMIT ?1 OFFSET ?2",
            page_size,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching runs")
    }

    async fn fetch_within(&self, start_time: i64, stop_time: i64) -> anyhow::Result<Vec<Run>> {
        sqlx::query_as!(
            Run,
            "SELECT * FROM run WHERE start_time <= ?1 AND stop_time >= ?2",
            stop_time,
            start_time
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching run")
    }

    async fn persist(&self, run: &Run) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO run (id, start_time, stop_time) VALUES (?1, ?2, ?3) 
            ON CONFLICT(id) DO UPDATE SET start_time=excluded.start_time, stop_time=excluded.stop_time;
            "#,
            run.id,
            run.start_time,
            run.stop_time
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .context("Error inserting scenario into db.")
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
impl RunDao for RemoteDao {
    async fn fetch(&self, _page_size: u32, _page: u32) -> anyhow::Result<Vec<Run>> {
        todo!()
    }

    async fn fetch_within(&self, _start_time: i64, _stop_time: i64) -> anyhow::Result<Vec<Run>> {
        todo!()
    }

    async fn persist(&self, run: &Run) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/run", self.base_url))
            .json(run)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting scenario to remote server")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations", fixtures("../../fixtures/runs.sql"))]
    async fn fetch_within_should_work(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let run_service = LocalDao::new(pool.clone());

        // fetch the runs between ... and ...
        let runs = run_service
            .fetch_within(1717507590000, 1717507601000)
            .await?;

        let run_ids = runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>();
        assert_eq!(run_ids, vec!["1"]);

        // fetch the runs between ... and ...
        let runs = run_service
            .fetch_within(1717507590000, 1717507795000)
            .await?;

        let run_ids = runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>();
        assert_eq!(run_ids, vec!["1", "2", "3"]);

        Ok(())
    }
}
