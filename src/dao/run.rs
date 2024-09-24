use crate::entities::run;
use anyhow::{self, Context};
use sea_orm::*;

pub async fn fetch(id: i32, db: &DatabaseConnection) -> anyhow::Result<run::Model> {
    run::Entity::find()
        .filter(run::Column::Id.eq(id))
        .one(db)
        .await?
        .context(format!("Error fetching run with id {}", id))
}
