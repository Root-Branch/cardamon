/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Context;
use async_trait::async_trait;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct ScenarioIteration {
    pub run_id: String,
    pub scenario_name: String,
    pub iteration: i64,
    pub start_time: i64,
    pub stop_time: i64,
}
impl ScenarioIteration {
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
pub trait ScenarioIterationDao {
    async fn fetch_last(
        &self,
        scenario_name: &str,
        n: u32,
    ) -> anyhow::Result<Vec<ScenarioIteration>>;
    async fn persist(&self, scenario_iteration: &ScenarioIteration) -> anyhow::Result<()>;
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
impl ScenarioIterationDao for LocalDao {
    async fn fetch_last(
        &self,
        scenario_name: &str,
        n: u32,
    ) -> anyhow::Result<Vec<ScenarioIteration>> {
        sqlx::query_as!(
            ScenarioIteration,
            r#"
            SELECT * 
            FROM scenario_iteration 
            WHERE scenario_name = ?1 AND run_id in (
                SELECT run_id 
                FROM scenario_iteration 
                WHERE scenario_name = ?1 
                GROUP BY run_id 
                ORDER BY start_time ASC
                LIMIT ?2
            )
            "#,
            scenario_name,
            n
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching scenarios")
    }

    async fn persist(&self, scenario_iteration: &ScenarioIteration) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO scenario_iteration (run_id, scenario_name, iteration, start_time, stop_time) VALUES (?1, ?2, ?3, ?4, ?5)", 
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
impl ScenarioIterationDao for RemoteDao {
    async fn fetch_last(
        &self,
        _scenario_name: &str,
        _n: u32,
    ) -> anyhow::Result<Vec<ScenarioIteration>> {
        todo!()
    }

    async fn persist(&self, scenario_iteration: &ScenarioIteration) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/scenario", self.base_url))
            .json(scenario_iteration)
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

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../fixtures/scenario_iterations.sql")
    )]
    async fn fetch_last_should_work(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let scenario_service = LocalDao::new(pool.clone());

        // fetch the latest scenario_1 run
        let scenario_iterations = scenario_service.fetch_last("scenario_1", 1).await?;

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
        let scenario_iterations = scenario_service.fetch_last("scenario_3", 2).await?;

        let run_ids = scenario_iterations
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec!["1", "1", "1", "2", "2", "2"]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.iteration)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1, 2, 3, 1, 2, 3]);

        Ok(())
    }
}
