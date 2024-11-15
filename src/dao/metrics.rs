use crate::entities::metrics;
use anyhow::{self, Context};
use sea_orm::*;

pub async fn fetch_within(
    run_id: &str,
    from: i64,
    to: Option<i64>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<metrics::Model>> {
    let query = metrics::Entity::find().filter(match to {
        Some(to) => Condition::all()
            .add(metrics::Column::RunId.eq(run_id))
            .add(metrics::Column::TimeStamp.gte(from))
            .add(metrics::Column::TimeStamp.lte(to)),
        None => Condition::all()
            .add(metrics::Column::RunId.eq(run_id))
            .add(metrics::Column::TimeStamp.gte(from)),
    });

    query.all(db).await.context(format!(
        "Error fetching metrics gathered between: {} and {:?}",
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
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/metrics.sql",
            ],
            &db,
        )
        .await?;

        let metrics =
            dao::metrics::fetch_within("1", 1717507600000, Some(1717507600200), &db).await?;

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
