use super::pagination::Page;
use crate::{
    dao::pagination::Pages,
    entities::iteration::{self, Entity as Iteration},
};
use anyhow::{self, Context};
use sea_orm::*;
use sea_query::{Expr, Query};
use tracing::trace;

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct RunId {
    pub run_id: i32,
}

pub async fn fetch_runs_all(
    scenarios: &Vec<String>,
    page: Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, Pages)> {
    trace!("page = {:?}", page);
    let sub_query = Query::select()
        .column(iteration::Column::RunId)
        .from(iteration::Entity)
        .to_owned();

    let query = iteration::Entity::find()
        .filter(iteration::Column::ScenarioName.is_in(scenarios))
        .filter(iteration::Column::RunId.in_subquery(sub_query))
        .order_by_desc(iteration::Column::StartTime)
        .group_by(iteration::Column::RunId);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await?;
            trace!("count = {}", count);
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query.paginate(db, page.size).fetch_page(page.num).await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

/// Return all iterations for the given scenario in the given date range. Page the results.
pub async fn fetch_runs_in_range(
    scenarios: &Vec<String>,
    from: i64,
    to: i64,
    page: Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, Pages)> {
    let query = iteration::Entity::find()
        .filter(
            Condition::all()
                .add(iteration::Column::ScenarioName.is_in(scenarios))
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_desc(iteration::Column::StartTime);

    match page {
        Some(page) => {
            let count = query.clone().count(db).await?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query.paginate(db, page.size).fetch_page(page.num).await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let res = query.all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

pub async fn fetch_runs_last_n(
    scenarios: &Vec<String>,
    last_n: u64,
    page: Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, Pages)> {
    if scenarios.is_empty() {
        return Err(anyhow::anyhow!("Cannot get runs for no scenarios!"));
    }

    match page {
        Some(page) => {
            if scenarios.is_empty() {
                return Err(anyhow::anyhow!(
                    "Unable to paginate over runs if multiple scenarios are selected!"
                ));
            }

            // SELECT *
            //         FROM iteration
            //         WHERE scenario_name IN ?1 AND run_id IN (
            //             SELECT run_id
            //             FROM iteration
            //             WHERE scenario_name IN ?1
            //             GROUP BY run_id
            //             ORDER BY start_time DESC
            //             LIMIT ?2
            //         )
            let scenario = scenarios.first().unwrap();

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

            let count = query.clone().count(db).await?;
            let page_count = (count as f64 / page.size as f64).ceil() as u64;

            let res = query.paginate(db, page.size).fetch_page(page.num).await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let mut res = vec![];
            for scenario in scenarios {
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
                let mut iterations = query.all(db).await?;
                res.append(&mut iterations);
            }

            Ok((res, Pages::NotRequired))
        }
    }
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
        setup_fixtures(
            &[
                "./fixtures/power_curves.sql",
                "./fixtures/cpus.sql",
                "./fixtures/runs.sql",
                "./fixtures/iterations.sql",
            ],
            &db,
        )
        .await?;

        // fetch the latest scenario_1 run
        let (scenario_iterations, _) =
            dao::iteration::fetch_runs_last_n(&vec!["scenario_1".to_string()], 1, None, &db)
                .await?;

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
        let (scenario_iterations, _) =
            dao::iteration::fetch_runs_last_n(&vec!["scenario_3".to_string()], 2, None, &db)
                .await?;

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
