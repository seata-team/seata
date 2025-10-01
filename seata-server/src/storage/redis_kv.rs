use super::KV;
use anyhow::Result;
use async_trait::async_trait;
use redis::{AsyncCommands, Client};
use std::time::Duration;

#[derive(Clone)]
pub struct RedisKv {
    client: Client,
}

impl RedisKv {
    pub fn new(url: &str) -> Result<Self> {
        let client = Client::open(url)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl KV for RedisKv {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let fut = conn.get::<_, Option<Vec<u8>>>(key);
        let v = tokio::time::timeout(Duration::from_secs(2), fut).await??;
        Ok(v)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let fut = conn.set::<_, _, ()>(key, value);
        tokio::time::timeout(Duration::from_secs(2), fut).await??;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let fut = conn.del::<_, ()>(key);
        tokio::time::timeout(Duration::from_secs(2), fut).await??;
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> Result<Vec<Vec<u8>>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        // use SCAN with MATCH
        let pattern = format!("{}*", String::from_utf8_lossy(prefix));
        let mut cursor: u64 = 0;
        let mut out = Vec::new();
        let batch_size = 256usize;
        loop {
            let mut scan_cmd = redis::cmd("SCAN");
            scan_cmd.cursor_arg(cursor).arg("MATCH").arg(&pattern).arg("COUNT").arg(1000);
            let fut = scan_cmd.query_async::<_, (u64, Vec<String>)>(&mut conn);
            let (next, keys) = tokio::time::timeout(Duration::from_secs(3), fut).await??;
            cursor = next;
            if keys.is_empty() { if cursor == 0 { break; } else { continue; } }
            // Batch MGET in chunks to reduce round-trips
            for chunk in keys.chunks(batch_size) {
                let mut p = redis::pipe();
                p.cmd("MGET").arg(chunk);
                let fut = p.query_async::<_, Vec<Option<Vec<u8>>>>(&mut conn);
                let values = tokio::time::timeout(Duration::from_secs(3), fut).await.unwrap_or(Ok(Vec::new()))?;
                for v in values.into_iter().flatten() {
                    out.push(v);
                    if out.len() >= limit { return Ok(out); }
                }
            }
            if cursor == 0 || out.len() >= limit { break; }
        }
        Ok(out)
    }
}

