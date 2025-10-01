use super::KV;
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection, Set, Statement, DbBackend, IntoActiveModel, QuerySelect, ConnectOptions};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "kv")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub k: Vec<u8>,
    pub v: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone)]
pub struct SeaKv {
    db: DatabaseConnection,
}

impl SeaKv {
    pub async fn mysql_with_pool(dsn: &str, max_open: Option<u32>, max_idle: Option<u32>, max_life_min: Option<u64>) -> Result<Self> {
        let mut opt = ConnectOptions::new(dsn.to_string());
        if let Some(m) = max_open { opt.max_connections(m); } else { opt.max_connections(50); }
        if let Some(m) = max_idle { opt.min_connections(m); } else { opt.min_connections(5); }
        opt.connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(30))
            .sqlx_logging(false);
        if let Some(_min) = max_life_min { /* max lifetime not exposed directly; skipped */ }
        let db = Database::connect(opt).await?;
        db.execute(Statement::from_string(DbBackend::MySql, String::from("CREATE TABLE IF NOT EXISTS kv (k VARBINARY(255) PRIMARY KEY, v LONGBLOB)"))).await?;
        Ok(Self { db })
    }
    pub async fn postgres_with_pool(dsn: &str, max_open: Option<u32>, max_idle: Option<u32>, max_life_min: Option<u64>) -> Result<Self> {
        let mut opt = ConnectOptions::new(dsn.to_string());
        if let Some(m) = max_open { opt.max_connections(m); } else { opt.max_connections(50); }
        if let Some(m) = max_idle { opt.min_connections(m); } else { opt.min_connections(5); }
        opt.connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(30))
            .sqlx_logging(false);
        if let Some(_min) = max_life_min { /* max lifetime not exposed directly; skipped */ }
        let db = Database::connect(opt).await?;
        db.execute(Statement::from_string(DbBackend::Postgres, String::from("CREATE TABLE IF NOT EXISTS kv (k bytea PRIMARY KEY, v bytea)"))).await?;
        Ok(Self { db })
    }
}

#[async_trait]
impl KV for SeaKv {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let res = Entity::find_by_id(key.to_vec()).one(&self.db).await?;
        Ok(res.map(|m| m.v))
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let existing = Entity::find_by_id(key.to_vec()).one(&self.db).await?;
        if let Some(mut m) = existing.map(|m| m.into_active_model()) {
            m.v = Set(value.to_vec());
            m.update(&self.db).await?;
        } else {
            let am = ActiveModel { k: Set(key.to_vec()), v: Set(value.to_vec()), ..Default::default() };
            am.insert(&self.db).await?;
        }
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        if let Some(m) = Entity::find_by_id(key.to_vec()).one(&self.db).await? { let _ = m.delete(&self.db).await?; }
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> Result<Vec<Vec<u8>>> {
        // Fallback: full scan with filter when DB doesn't support efficient prefix on binary keys
        let list = Entity::find().limit(limit as u64).all(&self.db).await?;
        Ok(list.into_iter().filter(|m| m.k.starts_with(prefix)).map(|m| m.v).collect())
    }
}

