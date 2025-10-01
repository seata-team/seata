use crate::domain::{GlobalTxn, GlobalStatus};
use crate::storage::KV;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait TxnRepo: Send + Sync {
    async fn save(&self, tx: &GlobalTxn) -> Result<()>;
    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>>;
    async fn list(&self, limit: usize, offset: usize, status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>>;
}

pub struct SledTxnRepo<K: KV> {
    kv: K,
}

impl<K: KV> SledTxnRepo<K> {
    pub fn new(kv: K) -> Self { Self { kv } }
}

#[async_trait]
impl<K: KV> TxnRepo for SledTxnRepo<K> {
    async fn save(&self, tx: &GlobalTxn) -> Result<()> {
        let key = format!("gid:{}", tx.gid);
        let val = serde_json::to_vec(tx)?;
        self.kv.put(key.as_bytes(), &val).await
    }

    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>> {
        let key = format!("gid:{}", gid);
        let Some(val) = self.kv.get(key.as_bytes()).await? else { return Ok(None) };
        let tx: GlobalTxn = serde_json::from_slice(&val)?;
        Ok(Some(tx))
    }
    async fn list(&self, limit: usize, offset: usize, status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>> {
        let vals = self.kv.scan_prefix(b"gid:", offset + limit).await?;
        let mut out = Vec::new();
        for v in vals.into_iter().skip(offset) {
            let tx: GlobalTxn = serde_json::from_slice(&v)?;
            if status.map(|s| s == tx.status).unwrap_or(true) { out.push(tx); }
        }
        Ok(out)
    }
}

pub struct DynTxnRepo {
    kv: Box<dyn KV>,
}

impl DynTxnRepo {
    pub fn new(kv: Box<dyn KV>) -> Self { Self { kv } }
}

#[async_trait]
impl TxnRepo for DynTxnRepo {
    async fn save(&self, tx: &GlobalTxn) -> Result<()> {
        let key = format!("gid:{}", tx.gid);
        let val = serde_json::to_vec(tx)?;
        self.kv.put(key.as_bytes(), &val).await
    }

    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>> {
        let key = format!("gid:{}", gid);
        let Some(val) = self.kv.get(key.as_bytes()).await? else { return Ok(None) };
        let tx: GlobalTxn = serde_json::from_slice(&val)?;
        Ok(Some(tx))
    }
    async fn list(&self, limit: usize, offset: usize, status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>> {
        let vals = self.kv.scan_prefix(b"gid:", offset + limit).await?;
        let mut out = Vec::new();
        for v in vals.into_iter().skip(offset) {
            let tx: GlobalTxn = serde_json::from_slice(&v)?;
            if status.map(|s| s == tx.status).unwrap_or(true) { out.push(tx); }
        }
        Ok(out)
    }
}

