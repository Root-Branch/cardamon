/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Context;
use async_trait::async_trait;
use nanoid::nanoid;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
pub struct ScenarioRun {
    pub id: String,
    pub cardamon_run_id: String,
    pub scenario_name: String,
    pub iteration: i64,
    pub start_time: i64,
    pub stop_time: i64,
}
impl ScenarioRun {
    pub fn new(
        cardamon_run_id: &str,
        scenario_name: &str,
        iteration: i64,
        start_time: i64,
        stop_time: i64,
    ) -> Self {
        Self {
            id: nanoid!(5),
            cardamon_run_id: String::from(cardamon_run_id),
            scenario_name: String::from(scenario_name),
            iteration,
            start_time,
            stop_time,
        }
    }
}

#[async_trait]
pub trait ScenarioRunDao {
    async fn fetch_last(&self, id: &str, n: u32) -> anyhow::Result<Vec<ScenarioRun>>;
    async fn fetch(&self, name: &str) -> anyhow::Result<Option<ScenarioRun>>;
    async fn persist(&self, model: &ScenarioRun) -> anyhow::Result<()>;
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
impl ScenarioRunDao for LocalDao {
    async fn fetch_last(&self, name: &str, n: u32) -> anyhow::Result<Vec<ScenarioRun>> {
        sqlx::query_as!(
            ScenarioRun,
            r#"
            SELECT * 
            FROM scenario_run 
            WHERE scenario_name = ?1 AND cardamon_run_id in (
                SELECT cardamon_run_id 
                FROM scenario_run 
                WHERE scenario_name = ?1 
                GROUP BY cardamon_run_id 
                ORDER BY start_time ASC
                LIMIT ?2
            )
            "#,
            name,
            n
        )
        .fetch_all(&self.pool)
        .await
        .context("Error fetching scenarios")
    }

    async fn fetch(&self, name: &str) -> anyhow::Result<Option<ScenarioRun>> {
        sqlx::query_as!(
            ScenarioRun,
            "SELECT * FROM scenario_run WHERE scenario_name = ?1",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .context("Error fetching scenario with id {id}")
    }

    async fn persist(&self, scenario: &ScenarioRun) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO scenario_run (id, cardamon_run_id, scenario_name, iteration, start_time, stop_time) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", 
            scenario.id,
            scenario.cardamon_run_id,
            scenario.scenario_name,
            scenario.iteration,
            scenario.start_time,
            scenario.stop_time)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .context("Error inserting scenario into db.")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM scenario_run WHERE id = ?1", id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .context("Error deleting scenario with id {id}")
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
impl ScenarioRunDao for RemoteDao {
    async fn fetch_last(&self, _name: &str, _n: u32) -> anyhow::Result<Vec<ScenarioRun>> {
        todo!()
    }

    async fn fetch(&self, id: &str) -> anyhow::Result<Option<ScenarioRun>> {
        self.client
            .get(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .json::<Option<ScenarioRun>>()
            .await
            .context("Error fetching scenario with id {id} from remote server")
    }

    async fn persist(&self, scenario: &ScenarioRun) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/scenario", self.base_url))
            .json(scenario)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error persisting scenario to remote server")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.client
            .delete(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
            .context("Error deleting scenario from remote server")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::panic;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[sqlx::test(migrations = "./migrations")]
    async fn test_local_scenario_service(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let scenario_service = LocalDao::new(pool.clone());

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

        let scenario = ScenarioRun::new("1", "my_scenario", 1, timestamp, timestamp + 10000);
        scenario_service.persist(&scenario).await?;

        match scenario_service.fetch(&scenario.id).await? {
            Some(fetched) => assert_eq!(fetched, scenario),
            None => panic!("scenario not found!"),
        }

        scenario_service.delete(&scenario.id).await?;

        if scenario_service.fetch(&scenario.id).await?.is_some() {
            panic!("scenario should not exist after delete!");
        }

        pool.close().await;
        Ok(())
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../fixtures/scenario_runs.sql")
    )]
    async fn fetch_last_should_work(pool: sqlx::SqlitePool) -> anyhow::Result<()> {
        let scenario_service = LocalDao::new(pool.clone());

        // fetch the latest scenario_1 run
        let scenario_runs = scenario_service.fetch_last("scenario_1", 1).await?;

        let cardamon_run_ids = scenario_runs
            .iter()
            .map(|run| run.cardamon_run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(cardamon_run_ids, vec!["1", "1", "1"]);

        let iterations = scenario_runs
            .iter()
            .map(|run| run.iteration)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1, 2, 3]);

        // fetch the last 2 scenario_3 runs
        let scenario_runs = scenario_service.fetch_last("scenario_3", 2).await?;

        let cardamon_run_ids = scenario_runs
            .iter()
            .map(|run| run.cardamon_run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(cardamon_run_ids, vec!["1", "1", "1", "2", "2", "2"]);

        let iterations = scenario_runs
            .iter()
            .map(|run| run.iteration)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1, 2, 3, 1, 2, 3]);

        Ok(())
    }
}
