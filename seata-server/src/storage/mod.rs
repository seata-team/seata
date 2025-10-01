use async_trait::async_trait;

#[async_trait]
pub trait KV: Send + Sync {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>>;
    async fn put(&self, key: &[u8], value: &[u8]) -> anyhow::Result<()>;
    async fn delete(&self, key: &[u8]) -> anyhow::Result<()>;
    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> anyhow::Result<Vec<Vec<u8>>>;
}

pub mod sled_kv;
pub mod redis_kv;
pub mod sea_kv;

