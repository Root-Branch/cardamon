use anyhow::Context;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct Run {
    pub id: String,
    pub start_time: i64,
    pub stop_time: Option<i64>,
}

impl Run {
    pub fn new(id: &str, start_time: i64, stop_time: Option<i64>) -> Self {
        Run {
            id: String::from(id),
            start_time,
            stop_time,
        }
    }
}

#[async_trait]
pub trait RunDao {
    /// Persist a run object to the db.
    async fn persist_run(&self, run: &Run) -> anyhow::Result<()>;
}

// //////////////////////////////////////
// LocalDao

#[derive(Clone, Debug)]
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
    async fn persist_run(&self, run: &Run) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO run (id, start_time, stop_time)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(id) DO UPDATE SET
                start_time = excluded.start_time,
                stop_time = excluded.stop_time
            "#,
            run.id,
            run.start_time,
            run.stop_time
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .context("Error inserting or updating run in db.")
    }
}

// //////////////////////////////////////
// RemoteDao

pub struct RemoteDao {
    _base_url: String,
    _client: reqwest::Client,
}

impl RemoteDao {
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.strip_suffix('/').unwrap_or(base_url);
        Self {
            _base_url: String::from(base_url),
            _client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl RunDao for RemoteDao {
    async fn persist_run(&self, _run: &Run) -> anyhow::Result<()> {
        todo!("Implement persist_run for RemoteDao")
    }
}

