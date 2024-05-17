use super::PersistenceService;
use anyhow::Context;
use nanoid::nanoid;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, sqlx::FromRow)]
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
        Self {
            id: nanoid!(5),
            cardamon_run_id: String::from(cardamon_run_id),
            scenario_name: String::from(scenario_name),
            start_time,
            stop_time,
        }
    }
}

////////////////////////////////////////
/// LocalPersistenceService
pub struct LocalPersistenceService<'a> {
    pub pool: &'a sqlx::SqlitePool,
}
impl<'a> LocalPersistenceService<'a> {
    pub fn new(pool: &'a sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}
impl<'a> PersistenceService<Scenario> for LocalPersistenceService<'a> {
    async fn fetch(&self, id: &str) -> anyhow::Result<Option<Scenario>> {
        sqlx::query_as!(Scenario, "SELECT * FROM scenario WHERE id = ?1", id)
            .fetch_optional(self.pool)
            .await
            .context("Error fetching scenario with id {id}")
    }

    async fn persist(&self, scenario: &Scenario) -> anyhow::Result<()> {
        sqlx::query!("INSERT INTO scenario (id, cardamon_run_id, scenario_name, start_time, stop_time) VALUES (?1, ?2, ?3, ?4, ?5)", 
            scenario.id,
            scenario.cardamon_run_id,
            scenario.scenario_name,
            scenario.start_time,
            scenario.stop_time)
            .execute(self.pool)
            .await
            .map(|_| ())
            .context("Error inserting scenario into db.")
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query!("DELETE FROM scenario WHERE id = ?1", id)
            .execute(self.pool)
            .await
            .map(|_| ())
            .context("Error deleting scenario with id {id}")
    }
}

////////////////////////////////////////
/// RemotePersistenceService
pub struct RemotePersistenceService {
    base_url: String,
    client: reqwest::Client,
}
impl RemotePersistenceService {
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.strip_suffix('/').unwrap_or(base_url);
        Self {
            base_url: String::from(base_url),
            client: reqwest::Client::new(),
        }
    }
}
impl PersistenceService<Scenario> for RemotePersistenceService {
    async fn fetch(&self, id: &str) -> anyhow::Result<Option<Scenario>> {
        self.client
            .get(format!("{}/scenario?id={id}", self.base_url))
            .send()
            .await?
            .json::<Option<Scenario>>()
            .await
            .context("Error fetching scenario with id {id} from remote server")
    }

    async fn persist(&self, scenario: &Scenario) -> anyhow::Result<()> {
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
        let scenario_service = LocalPersistenceService::new(&pool);

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

        let scenario = Scenario::new("1", "1", timestamp, timestamp + 10000);
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
}
