use crate::entities::metrics;
use anyhow::{self, Context};
use sea_orm::*;

pub async fn fetch_within(
    run: i32,
    from: i64,
    to: i64,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<metrics::Model>> {
    let query = metrics::Entity::find().filter(
        Condition::all()
            .add(metrics::Column::RunId.eq(run))
            .add(metrics::Column::TimeStamp.gte(from))
            .add(metrics::Column::TimeStamp.lte(to)),
    );

    query.all(db).await.context(format!(
        "Error fetching metrics gathered between: {} and {}",
        from, to
    ))
}

#[cfg(test)]
mod tests {
    use crate::{dao, db_connect, db_migrate, tests::setup_fixtures};
    use itertools::Itertools;

    #[tokio::test]
    async fn fetch_metrics_within() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(&["./fixtures/runs.sql", "./fixtures/metrics.sql"], &db).await?;

        let metrics = dao::metrics::fetch_within(1, 1717507600000, 1717507600200, &db).await?;

        assert_eq!(metrics.len(), 4);

        let process_names: Vec<&str> = metrics
            .iter()
            .map(|metric| metric.process_name.as_str())
            .unique()
            .collect();

        assert_eq!(process_names, vec!["yarn", "docker"]);

        Ok(())
    }
}
