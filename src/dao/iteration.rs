use super::pagination::Page;
use anyhow::{self, Context};
use entities::iteration::{self, Entity as Iteration};
use sea_orm::*;
use sea_query::{Expr, Query};

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct RunId {
    pub run_id: i32,
}

/// Return all iterations for the given scenario over all runs. Page the results.
pub async fn fetch_runs_all(
    scenario: &str,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<iteration::Model>> {
    let query = iteration::Entity::find()
        .filter(iteration::Column::ScenarioName.eq(scenario))
        .order_by_desc(iteration::Column::StartTime);

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    };

    res.context("Error fetching all iterations")
}

/// Return all iterations for the given scenario in the given date range. Page the results.
pub async fn fetch_runs_in_range(
    scenario: &str,
    from: i64,
    to: i64,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<iteration::Model>> {
    let query = iteration::Entity::find()
        .filter(
            Condition::all()
                .add(iteration::Column::ScenarioName.eq(scenario))
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_desc(iteration::Column::StartTime);

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    };

    res.context("Error fetching all iterations")
}

pub async fn fetch_runs_last_n(
    scenario: &str,
    last_n: u64,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<iteration::Model>> {
    // SELECT *
    //         FROM iteration
    //         WHERE scenario_name = ?1 AND run_id IN (
    //             SELECT run_id
    //             FROM iteration
    //             WHERE scenario_name = ?1
    //             GROUP BY run_id
    //             ORDER BY start_time DESC
    //             LIMIT ?2
    //         )
    let sub_query = Query::select()
        .expr(Expr::col(iteration::Column::RunId))
        .from(iteration::Entity)
        .cond_where(iteration::Column::ScenarioName.eq(scenario))
        .group_by_col(iteration::Column::RunId)
        .order_by(iteration::Column::StartTime, Order::Desc)
        .limit(last_n)
        .to_owned();

    let query = iteration::Entity::find().filter(
        Condition::all()
            .add(iteration::Column::ScenarioName.eq(scenario))
            .add(iteration::Column::RunId.in_subquery(sub_query)),
    );

    query.all(db).await.context(format!(
        "Error fetching iterations of the last {} runs of scenario {}",
        last_n, scenario
    ))
}

pub async fn fetch_unique_run_ids(
    scenario: &str,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<RunId>> {
    let query = iteration::Entity::find()
        .distinct_on([iteration::Column::RunId])
        .filter(iteration::Column::ScenarioName.eq(scenario))
        .order_by_desc(iteration::Column::StartTime)
        .into_partial_model::<RunId>();

    query.all(db).await.context("Error fetching unique run ids")
}

pub async fn fetch_by_scenario_and_run(
    scenario: &str,
    run_id: i32,
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<iteration::Model>> {
    let query = iteration::Entity::find()
        .filter(
            Condition::all()
                .add(iteration::Column::ScenarioName.eq(scenario))
                .add(iteration::Column::RunId.eq(run_id)),
        )
        .order_by_asc(iteration::Column::StartTime);

    query
        .all(db)
        .await
        .context("Error fetching iterations by scenario and run")
}

