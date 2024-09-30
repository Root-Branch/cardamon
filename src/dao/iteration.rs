use super::pagination::Page;
use crate::{
    dao::pagination::Pages,
    entities::iteration::{self, Entity as Iteration},
};
use anyhow::{self, Context};
use sea_orm::*;
use sea_query::{Alias, Query};
use tracing::trace;

#[derive(DerivePartialModel, FromQueryResult)]
#[sea_orm(entity = "Iteration")]
pub struct RunId {
    pub run_id: i32,
}

// VERIFIED (NoPage)
pub async fn fetch_runs_all(
    scenarios: &Vec<String>,
    page: Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, Pages)> {
    if scenarios.is_empty() {
        return Err(anyhow::anyhow!("Cannot get runs for no scenarios!"));
    }
    trace!("page = {:?}", page);

    match page {
        Some(Page { size, num }) => {
            if scenarios.len() > 1 {
                return Err(anyhow::anyhow!(
                    "Unable to paginate over runs if multiple scenarios are selected!"
                ));
            }

            // get count without pagination
            let count_query = iteration::Entity::find()
                .select_only()
                .select_column(iteration::Column::RunId)
                .distinct()
                .order_by(iteration::Column::StartTime, Order::Desc);
            let count = count_query.count(db).await?;
            let page_count = (count as f64 / size as f64).ceil() as u64;
            trace!("count = {}", count);

            // get data
            let sub_query = Query::select()
                .column(iteration::Column::RunId)
                .distinct()
                .from(iteration::Entity)
                .order_by(iteration::Column::StartTime, Order::Desc)
                .limit(size)
                .offset(size * num)
                .to_owned();
            let query = iteration::Entity::find()
                .filter(iteration::Column::ScenarioName.is_in(scenarios))
                .filter(iteration::Column::RunId.in_subquery(sub_query))
                .order_by_desc(iteration::Column::StartTime);

            // println!("\n [QUERY] {:?}", query.build(DatabaseBackend::Sqlite).sql);

            let res = query.all(db).await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let sub_query = Query::select()
                .column(iteration::Column::RunId)
                .distinct()
                .from(iteration::Entity)
                .order_by(iteration::Column::StartTime, Order::Desc)
                .to_owned();
            let query = iteration::Entity::find()
                .filter(iteration::Column::ScenarioName.is_in(scenarios))
                .filter(iteration::Column::RunId.in_subquery(sub_query))
                .order_by_desc(iteration::Column::StartTime);

            // println!("\n [QUERY] {:?}", query.build(DatabaseBackend::Sqlite).sql);

            let res = query.all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

// VERIFIED (NoPage)
/// Return all iterations for the given scenario in the given date range. Page the results.
pub async fn fetch_runs_in_range(
    scenarios: &Vec<String>,
    from: i64,
    to: i64,
    page: Option<Page>,
    db: &DatabaseConnection,
) -> anyhow::Result<(Vec<iteration::Model>, Pages)> {
    if scenarios.is_empty() {
        return Err(anyhow::anyhow!("Cannot get runs for no scenarios!"));
    }
    trace!("page = {:?}", page);

    match page {
        Some(Page { size, num }) => {
            if scenarios.len() > 1 {
                return Err(anyhow::anyhow!(
                    "Unable to paginate over runs if multiple scenarios are selected!"
                ));
            }

            // get count
            let count_query = iteration::Entity::find()
                .select_only()
                .select_column(iteration::Column::RunId)
                .distinct()
                .filter(iteration::Column::StopTime.gt(from))
                .filter(iteration::Column::StartTime.lte(to))
                .order_by(iteration::Column::StartTime, Order::Desc);
            let count = count_query.count(db).await?;
            let page_count = (count as f64 / size as f64).ceil() as u64;

            // get data
            let sub_query = Query::select()
                .column(iteration::Column::RunId)
                .distinct()
                .from(iteration::Entity)
                .cond_where(iteration::Column::StopTime.gte(from))
                .and_where(iteration::Column::StartTime.lte(to))
                .order_by(iteration::Column::StartTime, Order::Desc)
                .limit(size)
                .offset(size * num)
                .to_owned();
            let query = iteration::Entity::find()
                .filter(iteration::Column::ScenarioName.is_in(scenarios))
                .filter(iteration::Column::RunId.in_subquery(sub_query))
                .order_by_desc(iteration::Column::StartTime);

            let res = query.all(db).await?;

            Ok((res, Pages::Required(page_count)))
        }

        None => {
            let sub_query = Query::select()
                .column(iteration::Column::RunId)
                .distinct()
                .from(iteration::Entity)
                .cond_where(iteration::Column::StopTime.gte(from))
                .and_where(iteration::Column::StartTime.lte(to))
                .order_by(iteration::Column::StartTime, Order::Desc)
                .to_owned();
            let query = iteration::Entity::find()
                .filter(iteration::Column::ScenarioName.is_in(scenarios))
                .filter(iteration::Column::RunId.in_subquery(sub_query))
                .order_by_desc(iteration::Column::StartTime);

            println!("\n [QUERY] {}", query.build(DatabaseBackend::Sqlite).sql);

            let res = query.all(db).await?;
            Ok((res, Pages::NotRequired))
        }
    }
}

// VERIFIED (NoPage)
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
        Some(Page { size, num }) => {
            if scenarios.len() > 1 {
                return Err(anyhow::anyhow!(
                    "Unable to paginate over runs if multiple scenarios are selected!"
                ));
            }

            // get count
            let count_query = iteration::Entity::find()
                .select_only()
                .select_column(iteration::Column::RunId)
                .distinct()
                .order_by(iteration::Column::StartTime, Order::Desc)
                .limit(last_n);
            let count = count_query.count(db).await?;
            let page_count = (count as f64 / size as f64).ceil() as u64;

            // get data
            let sub_sub_query = Query::select()
                .column(iteration::Column::RunId)
                .distinct()
                .from(iteration::Entity)
                .order_by(iteration::Column::StartTime, Order::Desc)
                .limit(last_n)
                .to_owned();
            let sub_query = Query::select()
                .from_subquery(sub_sub_query, Alias::new("A"))
                .limit(size)
                .offset(size * num)
                .to_owned();
            let query = iteration::Entity::find()
                .filter(iteration::Column::ScenarioName.is_in(scenarios))
                .filter(iteration::Column::RunId.in_subquery(sub_query))
                .order_by_desc(iteration::Column::StartTime);

            let res = query.all(db).await?;

            Ok((res, Pages::Required(page_count)))

            // // SELECT *
            // //         FROM iteration
            // //         WHERE scenario_name IN ?1 AND run_id IN (
            // //             SELECT run_id
            // //             FROM iteration
            // //             WHERE scenario_name IN ?1
            // //             GROUP BY run_id
            // //             ORDER BY start_time DESC
            // //             LIMIT ?2
            // //         )
            // let scenario = scenarios.first().unwrap();
            //
            // let sub_query = Query::select()
            //     .expr(Expr::col(iteration::Column::RunId))
            //     .from(iteration::Entity)
            //     .cond_where(iteration::Column::ScenarioName.eq(scenario))
            //     .group_by_col(iteration::Column::RunId)
            //     .order_by(iteration::Column::StartTime, Order::Desc)
            //     .limit(last_n)
            //     .to_owned();
            //
            // let query = iteration::Entity::find().filter(
            //     Condition::all()
            //         .add(iteration::Column::ScenarioName.eq(scenario))
            //         .add(iteration::Column::RunId.in_subquery(sub_query)),
            // );
            //
            // let count = query.clone().count(db).await?;
            //
            // let res = query.paginate(db, page.size).fetch_page(page.num).await?;
            //
            // Ok((res, Pages::Required(page_count)))
        }

        None => {
            let mut res = vec![];
            for scenario in scenarios {
                let sub_query = Query::select()
                    .column(iteration::Column::RunId)
                    .distinct()
                    .from(iteration::Entity)
                    .cond_where(iteration::Column::ScenarioName.eq(scenario))
                    .order_by(iteration::Column::StartTime, Order::Desc)
                    .limit(last_n)
                    .to_owned();
                let query = iteration::Entity::find()
                    .filter(iteration::Column::ScenarioName.eq(scenario))
                    .filter(iteration::Column::RunId.in_subquery(sub_query))
                    .order_by_desc(iteration::Column::StartTime);

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
        assert_eq!(run_ids, vec![3, 3, 3, 2, 2, 2]);

        let iterations = scenario_iterations
            .iter()
            .map(|run| run.count)
            .collect::<Vec<_>>();
        assert_eq!(iterations, vec![3, 2, 1, 3, 2, 1]);

        Ok(())
    }
}
