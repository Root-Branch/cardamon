use super::pagination::{Page, Pages};
use crate::entities::iteration::{self, Entity as Iteration};
use anyhow::{self, Context};
use sea_orm::*;

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct ScenarioName {
    pub scenario_name: String,
}

pub async fn fetch(name: &String, db: &DatabaseConnection) -> anyhow::Result<Option<ScenarioName>> {
    iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(iteration::Column::ScenarioName.eq(name))
        .into_partial_model::<ScenarioName>()
        .one(db)
        .await
        .map_err(anyhow::Error::from)
}

pub async fn fetch_all(
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, Pages)> {
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .order_by_asc(iteration::Column::StartTime);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query
                .into_partial_model()
                .paginate(db, page.size)
                .fetch_page(page.num)
                .await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.into_partial_model().all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

pub async fn fetch_in_run(
    run: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, Pages)> {
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(iteration::Column::RunId.eq(run))
        .order_by_asc(iteration::Column::StartTime);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await.context(format!(
                "Error counting all scenarios executed in run {}",
                run
            ))?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query
                .into_partial_model()
                .paginate(db, page.size)
                .fetch_page(page.num)
                .await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.into_partial_model().all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

pub async fn fetch_in_range(
    from: i64,
    to: i64,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, Pages)> {
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(
            Condition::all()
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_asc(iteration::Column::StartTime);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await.context(format!(
                "Error counting scenarios in run between: from {}, to {}",
                from, to
            ))?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query
                .into_partial_model()
                .paginate(db, page.size)
                .fetch_page(page.num)
                .await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.into_partial_model().all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

pub async fn fetch_by_query(
    name: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<ScenarioName>, Pages)> {
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(iteration::Column::ScenarioName.like(name))
        .order_by_asc(iteration::Column::StartTime);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query
                .into_partial_model()
                .paginate(db, page.size)
                .fetch_page(page.num)
                .await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.into_partial_model().all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}
