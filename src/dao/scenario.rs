use super::pagination::Page;
use anyhow::{self, Context};
use entities::iteration::{self, Entity as Iteration};
use sea_orm::*;

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct ScenarioName {
    pub scenario_name: String,
}

pub async fn fetch_all(
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioName>> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .order_by_asc(iteration::Column::StartTime)
        .into_partial_model::<ScenarioName>();

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    };

    res.context("Error fetching all scenarios")
}

pub async fn fetch_in_run(
    run: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioName>> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(iteration::Column::RunId.eq(run))
        .order_by_asc(iteration::Column::StartTime)
        .into_partial_model::<ScenarioName>();

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,
        _ => query.all(db).await,
    };

    res.context(format!(
        "Error fetching all scenarios executed in run {}",
        run
    ))
}

pub async fn fetch_in_range(
    from: i64,
    to: i64,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioName>> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(
            Condition::all()
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_asc(iteration::Column::StartTime)
        .into_partial_model::<ScenarioName>();

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    };

    res.context(format!(
        "Error fetching scenarios run between: from {}, to {}",
        from, to
    ))
}

pub async fn fetch_by_name(
    name: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<ScenarioName>> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::ScenarioName])
        .filter(iteration::Column::ScenarioName.like(name))
        .order_by_asc(iteration::Column::StartTime)
        .into_partial_model::<ScenarioName>();

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    };

    res.context(format!("Error fetching scenarios by name: {}", name))
}