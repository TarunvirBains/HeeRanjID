use crate::Error;
use sqlx::Executor;
use sqlx::FromRow;

pub const SCHEMA_SQL: &str = include_str!("../sql/postgres/schema.sql");
pub const FETCH_NODE_SQL: &str = include_str!("../sql/postgres/queries/fetch_node.sql");
pub const FETCH_EPOCH_SQL: &str = include_str!("../sql/postgres/queries/fetch_epoch.sql");

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct HeerNode {
    pub node_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct HeerConfig {
    pub epoch: sqlx::types::time::PrimitiveDateTime,
}

pub fn validate_heer_node_id(node_id: i32) -> Result<u16, Error> {
    if !(0..=i32::from(crate::heer::HeerId::MAX_NODE_ID)).contains(&node_id) {
        return Err(Error::NodeIdOutOfRange {
            value: node_id.max(0) as u32,
            bits: crate::heer::HEER_NODE_ID_BITS,
        });
    }

    Ok(node_id as u16)
}

pub async fn install_schema<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::raw_sql(SCHEMA_SQL).execute(executor).await?;
    Ok(())
}

pub async fn fetch_node(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
) -> Result<Option<HeerNode>, sqlx::Error> {
    sqlx::query_as::<_, HeerNode>(FETCH_NODE_SQL)
        .bind(i32::from(node_id))
        .fetch_optional(executor)
        .await
}

pub async fn fetch_epoch(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
) -> Result<Option<sqlx::types::time::PrimitiveDateTime>, sqlx::Error> {
    let record = sqlx::query_as::<_, HeerConfig>(FETCH_EPOCH_SQL)
        .fetch_optional(executor)
        .await?;

    Ok(record.map(|row| row.epoch))
}
