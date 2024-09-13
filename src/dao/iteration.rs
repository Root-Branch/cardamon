use super::pagination::Page;
use crate::entities::iteration::{self, Entity as Iteration};
use anyhow::{self, Context};
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
) -> anyhow::Result<(Vec<iteration::Model>, u64)> {
    let query = iteration::Entity::find()
        .filter(iteration::Column::ScenarioName.eq(scenario))
        .order_by_desc(iteration::Column::StartTime);

    let count = query
        .clone()
        .distinct_on([iteration::Column::RunId])
        .count(db)
        .await
        .context("Error counting runs")?;

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    }
    .context("Error fetching all iterations")?;

    Ok((res, count))
}

/// Return all iterations for the given scenario in the given date range. Page the results.
pub async fn fetch_runs_in_range(
    scenario: &str,
    from: i64,
    to: i64,
    page: &Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, u64)> {
    let query = iteration::Entity::find()
        .filter(
            Condition::all()
                .add(iteration::Column::ScenarioName.eq(scenario))
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_desc(iteration::Column::StartTime);

    let count = query
        .clone()
        .count(db)
        .await
        .context("Error counting runs in range")?;

    let res = match page {
        Some(page) => query.paginate(db, page.size).fetch_page(page.num).await,

        _ => query.all(db).await,
    }
    .context("Error fetching all iterations")?;

    Ok((res, count))
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

pub async fn fetch_live(run_id: i32, db: &DatabaseConnection) -> anyhow::Result<iteration::Model> {
    iteration::Entity::find()
        .filter(iteration::Column::RunId.eq(run_id))
        .one(db)
        .await?
        .context(format!("Unable to find live iteration for run {}", run_id))
}

#[cfg(test)]
mod tests {
    use crate::{dao, db_connect, db_migrate, tests::setup_fixtures};

    #[tokio::test]
    async fn fetch_iterations_of_last_n_runs_for_schema() -> anyhow::Result<()> {
        let db = db_connect("sqlite::memory:", None).await?;
        db_migrate(&db).await?;
        setup_fixtures(&["./fixtures/runs.sql", "./fixtures/iterations.sql"], &db).await?;

        // fetch the latest scenario_1 run
        let scenario_iterations = dao::iteration::fetch_runs_last_n("scenario_1", 1, &db).await?;

        let run_ids = scenario_iterations
            .iter()
            .map(|run| run.run_id)
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec![1]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.count)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1]);

        // fetch the last 2 scenario_3 runs
        let scenario_iterations = dao::iteration::fetch_runs_last_n("scenario_3", 2, &db).await?;

        let run_ids = scenario_iterations
            .iter()
            .map(|run| run.run_id)
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec![2, 2, 2, 3, 3, 3]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.count)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![1, 2, 3, 1, 2, 3]);

        Ok(())
    }
}
