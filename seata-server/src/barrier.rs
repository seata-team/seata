use crate::domain::BarrierPhase;
use crate::storage::KV;
use anyhow::Result;
use async_trait::async_trait;

pub struct Barrier<K: KV> {
    kv: K,
}

impl<K: KV> Barrier<K> {
    pub fn new(kv: K) -> Self { Self { kv } }

    fn key(gid: &str, branch_id: &str, phase: BarrierPhase) -> String {
        let p = match phase { BarrierPhase::Try => "try", BarrierPhase::Confirm => "confirm", BarrierPhase::Cancel => "cancel" };
        format!("barrier:{}:{}:{}", gid, branch_id, p)
    }

    pub async fn exists(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool> {
        let key = Self::key(gid, branch_id, phase);
        Ok(self.kv.get(key.as_bytes()).await?.is_some())
    }

    pub async fn insert(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool> {
        let key = Self::key(gid, branch_id, phase);
        if self.kv.get(key.as_bytes()).await?.is_some() { return Ok(false); }
        self.kv.put(key.as_bytes(), &[]).await?;
        Ok(true)
    }
}

#[async_trait]
pub trait BarrierOps: Send + Sync {
    async fn exists(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool>;
    async fn insert(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool>;
}

#[async_trait]
impl<K: KV> BarrierOps for Barrier<K> {
    async fn exists(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool> {
        Barrier::exists(self, gid, branch_id, phase).await
    }
    async fn insert(&self, gid: &str, branch_id: &str, phase: BarrierPhase) -> Result<bool> {
        Barrier::insert(self, gid, branch_id, phase).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::KV;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone, Default)]
    struct TestKV {
        inner: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    }

    #[async_trait]
    impl KV for TestKV {
        async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self.inner.lock().await.get(key).cloned())
        }
        async fn put(&self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
            self.inner.lock().await.insert(key.to_vec(), value.to_vec());
            Ok(())
        }
        async fn delete(&self, key: &[u8]) -> anyhow::Result<()> {
            self.inner.lock().await.remove(key);
            Ok(())
        }
        async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> anyhow::Result<Vec<Vec<u8>>> {
            let map = self.inner.lock().await;
            let mut out = Vec::new();
            for (k, v) in map.iter() {
                if k.starts_with(prefix) {
                    out.push(v.clone());
                }
                if out.len() >= limit {
                    break;
                }
            }
            Ok(out)
        }
    }

    #[tokio::test]
    async fn test_barrier_key_generation() {
        let kv = TestKV::default();
        let _barrier = Barrier::new(kv);

        // Test key generation for different phases
        let try_key = Barrier::<TestKV>::key("gid1", "branch1", BarrierPhase::Try);
        assert_eq!(try_key, "barrier:gid1:branch1:try");

        let confirm_key = Barrier::<TestKV>::key("gid1", "branch1", BarrierPhase::Confirm);
        assert_eq!(confirm_key, "barrier:gid1:branch1:confirm");

        let cancel_key = Barrier::<TestKV>::key("gid1", "branch1", BarrierPhase::Cancel);
        assert_eq!(cancel_key, "barrier:gid1:branch1:cancel");
    }

    #[tokio::test]
    async fn test_barrier_insert_and_exists() {
        let kv = TestKV::default();
        let barrier = Barrier::new(kv);

        let gid = "test-gid";
        let branch_id = "test-branch";
        let phase = BarrierPhase::Try;

        // Initially should not exist
        assert!(!barrier.exists(gid, branch_id, phase).await.unwrap());

        // Insert should return true (new barrier)
        assert!(barrier.insert(gid, branch_id, phase).await.unwrap());

        // Now should exist
        assert!(barrier.exists(gid, branch_id, phase).await.unwrap());

        // Second insert should return false (already exists)
        assert!(!barrier.insert(gid, branch_id, phase).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_idempotency() {
        let kv = TestKV::default();
        let barrier = Barrier::new(kv);

        let gid = "test-gid";
        let branch_id = "test-branch";

        // Test Try phase
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Try).await.unwrap());
        assert!(!barrier.insert(gid, branch_id, BarrierPhase::Try).await.unwrap());

        // Test Confirm phase
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Confirm).await.unwrap());
        assert!(!barrier.insert(gid, branch_id, BarrierPhase::Confirm).await.unwrap());

        // Test Cancel phase
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Cancel).await.unwrap());
        assert!(!barrier.insert(gid, branch_id, BarrierPhase::Cancel).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_different_phases() {
        let kv = TestKV::default();
        let barrier = Barrier::new(kv);

        let gid = "test-gid";
        let branch_id = "test-branch";

        // Each phase should be independent
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Try).await.unwrap());
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Confirm).await.unwrap());
        assert!(barrier.insert(gid, branch_id, BarrierPhase::Cancel).await.unwrap());

        // All should exist
        assert!(barrier.exists(gid, branch_id, BarrierPhase::Try).await.unwrap());
        assert!(barrier.exists(gid, branch_id, BarrierPhase::Confirm).await.unwrap());
        assert!(barrier.exists(gid, branch_id, BarrierPhase::Cancel).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_different_branches() {
        let kv = TestKV::default();
        let barrier = Barrier::new(kv);

        let gid = "test-gid";
        let branch1 = "branch1";
        let branch2 = "branch2";

        // Different branches should be independent
        assert!(barrier.insert(gid, branch1, BarrierPhase::Try).await.unwrap());
        assert!(barrier.insert(gid, branch2, BarrierPhase::Try).await.unwrap());

        assert!(barrier.exists(gid, branch1, BarrierPhase::Try).await.unwrap());
        assert!(barrier.exists(gid, branch2, BarrierPhase::Try).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_different_gids() {
        let kv = TestKV::default();
        let barrier = Barrier::new(kv);

        let gid1 = "gid1";
        let gid2 = "gid2";
        let branch_id = "test-branch";

        // Different GIDs should be independent
        assert!(barrier.insert(gid1, branch_id, BarrierPhase::Try).await.unwrap());
        assert!(barrier.insert(gid2, branch_id, BarrierPhase::Try).await.unwrap());

        assert!(barrier.exists(gid1, branch_id, BarrierPhase::Try).await.unwrap());
        assert!(barrier.exists(gid2, branch_id, BarrierPhase::Try).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_ops_trait() {
        let kv = TestKV::default();
        let barrier: Box<dyn BarrierOps> = Box::new(Barrier::new(kv));

        let gid = "test-gid";
        let branch_id = "test-branch";
        let phase = BarrierPhase::Try;

        // Test through trait
        assert!(!barrier.exists(gid, branch_id, phase).await.unwrap());
        assert!(barrier.insert(gid, branch_id, phase).await.unwrap());
        assert!(barrier.exists(gid, branch_id, phase).await.unwrap());
        assert!(!barrier.insert(gid, branch_id, phase).await.unwrap());
    }

    #[tokio::test]
    async fn test_barrier_concurrent_access() {
        let kv = TestKV::default();
        let barrier = Arc::new(Barrier::new(kv));

        let gid = "test-gid";
        let branch_id = "test-branch";
        let phase = BarrierPhase::Try;

        // Test concurrent access
        let barrier1 = barrier.clone();
        let barrier2 = barrier.clone();

        let handle1 = tokio::spawn(async move {
            barrier1.insert(gid, branch_id, phase).await
        });

        let handle2 = tokio::spawn(async move {
            barrier2.insert(gid, branch_id, phase).await
        });

        let result1 = handle1.await.unwrap().unwrap();
        let result2 = handle2.await.unwrap().unwrap();

        // Only one should succeed
        assert!(result1 ^ result2); // XOR - exactly one should be true
    }
}

