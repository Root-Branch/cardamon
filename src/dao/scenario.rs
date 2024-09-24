use super::pagination::Page;
use crate::entities::iteration::{self, Entity as Iteration};
use anyhow::{self, Context};
use sea_orm::*;

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct ScenarioName {
    pub scenario_name: String,
}

pub async fn fetch_all(
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, u64)> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .order_by_asc(iteration::Column::StartTime);

    let count = query
        .clone()
        .count(db)
        .await
        .context("Error counting all scenarios")?;

    let query = query.into_partial_model();
    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    }
    .context("Error fetching all scenarios")?;

    Ok((res, count))
}

pub async fn fetch_in_run(
    run: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, u64)> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(iteration::Column::RunId.eq(run))
        .order_by_asc(iteration::Column::StartTime);

    let count = query.clone().count(db).await.context(format!(
        "Error counting all scenarios executed in run {}",
        run
    ))?;

    let query = query.into_partial_model::<ScenarioName>();
    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,
        _ => query.all(db).await,
    }
    .context(format!(
        "Error fetching all scenarios executed in run {}",
        run
    ))?;

    Ok((res, count))
}

pub async fn fetch_in_range(
    from: i64,
    to: i64,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, u64)> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(
            Condition::all()
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_asc(iteration::Column::StartTime);

    let count = query.clone().count(db).await.context(format!(
        "Error counting scenarios in run between: from {}, to {}",
        from, to
    ))?;

    let query = query.into_partial_model::<ScenarioName>();
    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    }
    .context(format!(
        "Error fetching scenarios run between: from {}, to {}",
        from, to
    ))?;

    Ok((res, count))
}

pub async fn fetch_by_name(
    name: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, u64)> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(iteration::Column::ScenarioName.like(name))
        .order_by_asc(iteration::Column::StartTime);

    let count = query
        .clone()
        .count(db)
        .await
        .context(format!("Error counting scenarios by name: {}", name))?;

    let query = query.into_partial_model::<ScenarioName>();
    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    }
    .context(format!("Error fetching scenarios by name: {}", name))?;

    Ok((res, count))
}
