use anyhow::{self, Context};
use async_trait::async_trait;
use sqlx::SqlitePool;

use super::pagination::Page;

#[async_trait]
pub trait ScenarioDao {
    /// Return all scenarios. Page the results
    async fn fetch_all(&self, page: &Option<Page>) -> anyhow::Result<Vec<String>>;

    /// Find all scenarios that were executed during the given run and return their names. Page the
    /// results.
    async fn fetch_in_run(&self, run: &str, page: &Option<Page>) -> anyhow::Result<Vec<String>>;

    /// Return all scenarios which were run in the given date range. Page the results.
    async fn fetch_in_range(
        &self,
        from: i64,
        to: i64,
        page: &Option<Page>,
    ) -> anyhow::Result<Vec<String>>;

    /// Return all scenarios whos name matches the given name. Page the results.
    async fn fetch_by_name(&self, name: &str, page: &Option<Page>) -> anyhow::Result<Vec<String>>;
}

pub struct LocalDao {
    pool: SqlitePool,
}
impl LocalDao {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl ScenarioDao for LocalDao {
    async fn fetch_all(&self, page: &Option<Page>) -> anyhow::Result<Vec<String>> {
        match &page {
            None => {
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration ORDER BY start_time"
                );
                query
                    .fetch_all(&self.pool)
                    .await
                    .context("Error fetching scenarios")
            }

            Some(page) => {
                let offset = page.offset();
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration ORDER BY start_time LIMIT ?1 OFFSET ?2",
                    page.size,
                    offset
                );

                query
                    .fetch_all(&self.pool)
                    .await
                    .context("Error fetching scenarios")
            }
        }
    }

    async fn fetch_in_run(&self, run: &str, page: &Option<Page>) -> anyhow::Result<Vec<String>> {
        match page {
            None => {
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE run_id = ?1 ORDER BY start_time",
                    run
                );
                query.fetch_all(&self.pool).await.context("")
            }

            Some(page) => {
                let offset = page.offset();
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE run_id = ?1 ORDER BY start_time LIMIT ?2 OFFSET ?3",
                    run,
                    page.size,
                    offset
                );
                query.fetch_all(&self.pool).await.context("")
            }
        }
    }

    async fn fetch_in_range(
        &self,
        from: i64,
        to: i64,
        page: &Option<Page>,
    ) -> anyhow::Result<Vec<String>> {
        match page {
            None => {
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE start_time <= ?1 AND stop_time >= ?2", 
                    to, from
                );
                query.fetch_all(&self.pool).await.context("")
            }

            Some(page) => {
                let offset = page.offset();
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE start_time <= ?1 AND stop_time >= ?2 LIMIT ?3 OFFSET ?4", 
                    to, from, page.size, offset
                );
                query.fetch_all(&self.pool).await.context("")
            }
        }
    }

    async fn fetch_by_name(&self, name: &str, page: &Option<Page>) -> anyhow::Result<Vec<String>> {
        match page {
            None => {
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE scenario_name LIKE ?1",
                    name
                );
                query.fetch_all(&self.pool).await.context("")
            }

            Some(page) => {
                let offset = page.offset();
                let query = sqlx::query_scalar!(
                    "SELECT DISTINCT scenario_name FROM iteration WHERE scenario_name LIKE ?1 LIMIT ?2 OFFSET ?3",
                    name,
                    page.size,
                    offset
                );
                query.fetch_all(&self.pool).await.context("")
            }
        }
    }
}

pub struct RemoteDao {
    _base_url: String,
}
impl RemoteDao {
    pub fn new(base_url: &str) -> Self {
        Self {
            _base_url: base_url.to_string(),
        }
    }
}
#[async_trait]
impl ScenarioDao for RemoteDao {
    async fn fetch_all(&self, _page: &Option<Page>) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    async fn fetch_in_run(&self, _run: &str, _page: &Option<Page>) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    async fn fetch_in_range(
        &self,
        _from: i64,
        _to: i64,
        _page: &Option<Page>,
    ) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    async fn fetch_by_name(
        &self,
        _name: &str,
        _page: &Option<Page>,
    ) -> anyhow::Result<Vec<String>> {
        todo!()
    }
}
