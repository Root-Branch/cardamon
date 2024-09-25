use crate::entities::cpu;
use anyhow::{self, Context};
use sea_orm::*;

pub async fn fetch_by_name(name: &str, db: &DatabaseConnection) -> anyhow::Result<cpu::Model> {
    cpu::Entity::find()
        .filter(cpu::Column::Name.eq(name))
        .one(db)
        .await?
        .context(format!("Error fetching CPU with name {}", name))
}
