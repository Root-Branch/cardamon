use super::pagination::Page;
use anyhow::Context;
use async_trait::async_trait;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct Iteration {
    pub run_id: String,
    pub scenario_name: String,
    pub iteration: i64,
    pub start_time: i64,
    pub stop_time: i64,
}
impl Iteration {
    pub fn new(
        run_id: &str,
        scenario_name: &str,
        iteration: i64,
        start_time: i64,
        stop_time: i64,
    ) -> Self {
        Self {
            run_id: String::from(run_id),
            scenario_name: String::from(scenario_name),
            iteration,
            start_time,
            stop_time,
        }
    }
}

#[async_trait]
pub trait IterationDao {
    /// Return all iterations for the given scenario over all runs. Page the results.
    async fn fetch_runs_all(&self, scenario: &str, page: &Page) -> anyhow::Result<Vec<Iteration>>;

    /// Return all iterations for the given scenario in the given date range. Page the results.
    async fn fetch_runs_in_range(
        &self,
        scenario: &str,
        from: i64,
        to: i64,
        page: &Page,
    ) -> anyhow::Result<Vec<Iteration>>;

    /// Return the iterations in the last n runs of the given scenario.
    async fn fetch_runs_last_n(&self, scenario: &str, n: u32) -> anyhow::Result<Vec<Iteration>>;

    /// Save an iteration to the db.
    async fn persist(&self, iteration: &Iteration) -> anyhow::Result<()>;
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
impl IterationDao for LocalDao {
    async fn fetch_runs_all(&self, scenario: &str, page: &Page) -> anyhow::Result<Vec<Iteration>> {
        let offset = page.offset();
        sqlx::query_as!(
            Iteration,
            r#"
            SELECT * FROM iteration 
            WHERE scenario_name = ?1 
            ORDER BY start_time DESC 
            LIMIT ?2 OFFSET ?3
            "#,
            scenario,
            page.size,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching iterations")
    }

    async fn fetch_runs_in_range(
        &self,
        scenario: &str,
        from: i64,
        to: i64,
        page: &Page,
    ) -> anyhow::Result<Vec<Iteration>> {
        let offset = page.offset();
        sqlx::query_as!(
            Iteration,
            r#"
            SELECT * FROM iteration 
            WHERE scenario_name = ?1 AND start_time <= ?2 AND stop_time >= ?3 
            ORDER BY start_time DESC 
            LIMIT ?4 OFFSET ?5
            "#,
            scenario,
            from,
            to,
            page.size,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching iterations")
    }

    async fn fetch_runs_last_n(&self, scenario: &str, n: u32) -> anyhow::Result<Vec<Iteration>> {
        sqlx::query_as!(
            Iteration,
            r#"
            SELECT *
            FROM iteration
            WHERE scenario_name = ?1 AND run_id IN (
                SELECT run_id
                FROM iteration
                WHERE scenario_name = ?1
                GROUP BY run_id
                ORDER BY start_time DESC
                LIMIT ?2
            )
            "#,
            scenario,
            n
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching iterations")
    }

    async fn persist(&self, scenario_iteration: &Iteration) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO iteration (run_id, scenario_name, iteration, start_time, stop_time) VALUES (?1, ?2, ?3, ?4, ?5)",
            scenario_iteration.run_id,
            scenario_iteration.scenario_name,
            scenario_iteration.iteration,
            scenario_iteration.start_time,
            scenario_iteration.stop_time)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .context("Error inserting scenario into db.")
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
impl IterationDao for RemoteDao {
    async fn fetch_runs_all(
        &self,
        _scenario: &str,
        _page: &Page,
    ) -> anyhow::Result<Vec<Iteration>> {
        todo!()
    }

    async fn fetch_runs_in_range(
        &self,
        _scenario: &str,
        _from: i64,
        _to: i64,
        _page: &Page,
    ) -> anyhow::Result<Vec<Iteration>> {
        todo!()
    }

    async fn fetch_runs_last_n(&self, _scenario: &str, _n: u32) -> anyhow::Result<Vec<Iteration>> {
        todo!()
    }

    async fn persist(&self, _iteration: &Iteration) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../fixtures/runs.sql", "../../fixtures/iterations.sql")
    )]
    async fn fetch_last_should_work(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let scenario_service = LocalDao::new(pool.clone());

        // fetch the latest scenario_1 run
        let scenario_iterations = scenario_service.fetch_runs_last_n("scenario_1", 1).await?;

        let run_ids = scenario_iterations
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec!["1"]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.iteration)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1]);

        // fetch the last 2 scenario_3 runs
        let scenario_iterations = scenario_service.fetch_runs_last_n("scenario_3", 2).await?;

        let run_ids = scenario_iterations
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec!["2", "2", "2", "3", "3", "3"]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.iteration)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1, 2, 3, 1, 2, 3]);

        Ok(())
    }
}
