use super::KV;
use anyhow::Result;
use async_trait::async_trait;
use redis::{AsyncCommands, Client};

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
        let v: Option<Vec<u8>> = conn.get(key).await?;
        Ok(v)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        conn.set::<_, _, ()>(key, value).await?;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> Result<Vec<Vec<u8>>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        // use SCAN with MATCH
        let pattern = format!("{}*", String::from_utf8_lossy(prefix));
        let mut cursor: u64 = 0;
        let mut out = Vec::new();
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN").cursor_arg(cursor).arg("MATCH").arg(&pattern).arg("COUNT").arg(1000).query_async(&mut conn).await?;
            cursor = next;
            for k in keys {
                if out.len() >= limit { return Ok(out); }
                let v: Option<Vec<u8>> = conn.get(&k).await?;
                if let Some(v) = v { out.push(v); }
            }
            if cursor == 0 || out.len() >= limit { break; }
        }
        Ok(out)
    }
}

