use super::pagination::{Page, Pages};
use crate::entities::iteration::{self, Entity as Iteration};
use anyhow::{self, Context};
use sea_orm::*;
use tracing::trace;

#[derive(DerivePartialModel, FromQueryResult, Debug)]
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
    trace!("page = {:?}", page);
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .order_by_desc(iteration::Column::StartTime);

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
    trace!("page = {:?}", page);
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(iteration::Column::RunId.eq(run))
        .order_by_desc(iteration::Column::StartTime);

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
    trace!("page = {:?}", page);
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(
            Condition::all()
                .add(iteration::Column::StopTime.gt(from))
                .add(iteration::Column::StartTime.lt(to)),
        )
        .order_by_desc(iteration::Column::StartTime);

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
    trace!("page = {:?}", page);
    let query = iteration::Entity::find()
        .select_only()
        .select_column(iteration::Column::ScenarioName)
        .distinct()
        .filter(iteration::Column::ScenarioName.like(name))
        .order_by_desc(iteration::Column::StartTime);

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

#[cfg(test)]
mod tests {
    use crate::{
        data::{dataset::LiveDataFilter, dataset_builder::DatasetBuilder},
        db_connect, db_migrate,
        tests::setup_fixtures,
    };

    #[tokio::test]
    async fn building_dataset_for_single_scenario() -> anyhow::Result<()> {
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

        let builder = DatasetBuilder::new();
        let b01 = builder.scenario("scenario_3").all().runs_all().all();
        let dataset = b01.build(&db).await?;
        for scenario_dataset in dataset.by_scenario(LiveDataFilter::IncludeLive) {
            let run_datasets = scenario_dataset.by_run();
            assert_eq!(run_datasets.len(), 3);

            assert_eq!(run_datasets.get(0).unwrap().run_id(), 3);
            assert_eq!(run_datasets.get(1).unwrap().run_id(), 2);
            assert_eq!(run_datasets.get(2).unwrap().run_id(), 1);
        }

        let b02 = builder
            .scenario("scenario_3")
            .all()
            .runs_in_range(0, 1717507699000)
            .all();
        let dataset = b02.build(&db).await?;
        for scenario_dataset in dataset.by_scenario(LiveDataFilter::IncludeLive) {
            let run_datasets = scenario_dataset.by_run();
            assert_eq!(run_datasets.len(), 2);

            assert_eq!(run_datasets.get(0).unwrap().run_id(), 2);
            assert_eq!(run_datasets.get(1).unwrap().run_id(), 1);
        }

        let b03 = builder.scenario("scenario_3").all().last_n_runs(1).all();
        let dataset = b03.build(&db).await?;
        for scenario_dataset in dataset.by_scenario(LiveDataFilter::IncludeLive) {
            let run_datasets = scenario_dataset.by_run();
            assert_eq!(run_datasets.len(), 1);

            assert_eq!(run_datasets.get(0).unwrap().run_id(), 3);
        }

        Ok(())
    }

    #[tokio::test]
    async fn build_dataset_for_all_scenarios() -> anyhow::Result<()> {
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

        let builder = DatasetBuilder::new();
        let b04 = builder.scenarios_all().all().runs_all().all();
        let dataset = b04.build(&db).await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        assert_eq!(
            scenario_datasets.get(0).unwrap().scenario_name(),
            "scenario_3"
        );
        assert_eq!(
            scenario_datasets.get(1).unwrap().scenario_name(),
            "scenario_2"
        );
        assert_eq!(
            scenario_datasets.get(2).unwrap().scenario_name(),
            "scenario_1"
        );

        let b05 = builder
            .scenarios_all()
            .all()
            .runs_in_range(0, 1717507699000)
            .all();
        let dataset = b05.build(&db).await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        assert_eq!(
            scenario_datasets.get(0).unwrap().scenario_name(),
            "scenario_3"
        );
        assert_eq!(scenario_datasets.get(0).unwrap().by_run().len(), 2);
        assert_eq!(
            scenario_datasets.get(1).unwrap().scenario_name(),
            "scenario_2"
        );
        assert_eq!(scenario_datasets.get(1).unwrap().by_run().len(), 2);

        let b06 = builder.scenarios_all().all().last_n_runs(1).all();
        let dataset = b06.build(&db).await?;
        let scenario_datasets = dataset.by_scenario(LiveDataFilter::IncludeLive);
        assert_eq!(
            scenario_datasets.get(0).unwrap().scenario_name(),
            "scenario_3"
        );
        assert_eq!(scenario_datasets.get(0).unwrap().by_run().len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn build_dataset_for_scenarios_in_run() -> anyhow::Result<()> {
        let builder = DatasetBuilder::new();
        let b07 = builder.scenarios_in_run(1).all().runs_all().all();
        let b08 = builder.scenarios_in_run(1).all().runs_in_range(0, 1).all();
        let b09 = builder.scenarios_in_run(1).all().last_n_runs(1).all();
        Ok(())
    }

    #[tokio::test]
    async fn build_dataset_for_scenarios_by_name() -> anyhow::Result<()> {
        let builder = DatasetBuilder::new();
        let b10 = builder.scenarios_by_name("").all().runs_all().all();
        let b11 = builder
            .scenarios_by_name("")
            .all()
            .runs_in_range(0, 1)
            .all();
        let b12 = builder.scenarios_by_name("").all().last_n_runs(1).all();
        Ok(())
    }
}
