use std::path::PathBuf;

use super::KV;
use anyhow::Result;
use async_trait::async_trait;

pub struct SledKv {
    db: sled::Db,
}

impl SledKv {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let db = sled::open(path.into())?;
        Ok(Self { db })
    }
}

#[async_trait]
impl KV for SledKv {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let db = self.db.clone();
        let key = key.to_vec();
        tokio::task::spawn_blocking(move || db.get(key)).await??.map(|ivec| Ok(ivec.to_vec())).transpose()
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();
        let value = value.to_vec();
        tokio::task::spawn_blocking(move || db.insert(key, value)).await??;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();
        tokio::task::spawn_blocking(move || db.remove(key)).await??;
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> Result<Vec<Vec<u8>>> {
        let db = self.db.clone();
        let prefix = prefix.to_vec();
        let out: Vec<Vec<u8>> = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            for item in db.scan_prefix(prefix).take(limit) {
                let (_k, v) = item?;
                out.push(v.to_vec());
            }
            Ok::<_, sled::Error>(out)
        }).await??;
        Ok(out)
    }
}

