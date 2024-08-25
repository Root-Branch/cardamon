use anyhow::{self, Context};
use entities::metrics;
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
