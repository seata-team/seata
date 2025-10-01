use crate::storage::{sled_kv::SledKv, redis_kv::RedisKv, sea_kv::SeaKv, KV};
use crate::domain::{GlobalTxn, BranchTxn, GlobalStatus, BranchStatus};
use crate::repo::{TxnRepo, DynTxnRepo};
use std::sync::Arc;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sled_kv_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        
        // Test basic operations
        assert!(kv.get(b"key1").await.unwrap().is_none());
        
        kv.put(b"key1", b"value1").await.unwrap();
        assert_eq!(kv.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));
        
        kv.delete(b"key1").await.unwrap();
        assert!(kv.get(b"key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sled_kv_scan_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        
        // Insert multiple keys with prefix
        kv.put(b"prefix:key1", b"value1").await.unwrap();
        kv.put(b"prefix:key2", b"value2").await.unwrap();
        kv.put(b"other:key3", b"value3").await.unwrap();
        
        // Scan with prefix
        let results = kv.scan_prefix(b"prefix:", 10).await.unwrap();
        assert_eq!(results.len(), 2);
        
        // Scan with limit
        let results = kv.scan_prefix(b"prefix:", 1).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_sled_repo_operations() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        let now = chrono::Utc::now().timestamp();
        let txn = GlobalTxn {
            gid: "test-gid".to_string(),
            mode: "saga".to_string(),
            status: GlobalStatus::Submitted,
            payload: b"test payload".to_vec(),
            branches: vec![
                BranchTxn {
                    branch_id: "branch1".to_string(),
                    action: "http://example.com/action1".to_string(),
                    status: BranchStatus::Prepared,
                },
                BranchTxn {
                    branch_id: "branch2".to_string(),
                    action: "http://example.com/action2".to_string(),
                    status: BranchStatus::Succeed,
                },
            ],
            updated_unix: now,
            created_unix: now,
        };
        
        // Save transaction
        repo.save(&txn).await.unwrap();
        
        // Load transaction
        let loaded = repo.load("test-gid").await.unwrap().unwrap();
        assert_eq!(loaded.gid, "test-gid");
        assert_eq!(loaded.branches.len(), 2);
        assert_eq!(loaded.branches[0].branch_id, "branch1");
        assert_eq!(loaded.branches[1].branch_id, "branch2");
        
        // List transactions
        let list = repo.list(10, 0, None).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].gid, "test-gid");
        
        // List with status filter
        let list = repo.list(10, 0, Some(GlobalStatus::Submitted)).await.unwrap();
        assert_eq!(list.len(), 1);
        
        let list = repo.list(10, 0, Some(GlobalStatus::Committed)).await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_redis_kv_operations() {
        // Skip if Redis is not available
        if std::env::var("REDIS_URL").is_err() {
            println!("Skipping Redis tests - REDIS_URL not set");
            return;
        }
        
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
        let kv = RedisKv::new(&redis_url).unwrap();
        
        // Test basic operations
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
        
        kv.put(b"test:key1", b"value1").await.unwrap();
        assert_eq!(kv.get(b"test:key1").await.unwrap(), Some(b"value1".to_vec()));
        
        kv.delete(b"test:key1").await.unwrap();
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
        
        // Clean up
        kv.delete(b"test:key1").await.unwrap();
    }

    #[tokio::test]
    async fn test_redis_repo_operations() {
        // Skip if Redis is not available
        if std::env::var("REDIS_URL").is_err() {
            println!("Skipping Redis repo tests - REDIS_URL not set");
            return;
        }
        
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
        let kv = RedisKv::new(&redis_url).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        let now = chrono::Utc::now().timestamp();
        let txn = GlobalTxn {
            gid: "redis-test-gid".to_string(),
            mode: "saga".to_string(),
            status: GlobalStatus::Submitted,
            payload: b"redis test payload".to_vec(),
            branches: vec![],
            updated_unix: now,
            created_unix: now,
        };
        
        // Save and load
        repo.save(&txn).await.unwrap();
        let loaded = repo.load("redis-test-gid").await.unwrap().unwrap();
        assert_eq!(loaded.gid, "redis-test-gid");
        
        // Clean up
        repo.load("redis-test-gid").await.unwrap();
    }

    #[tokio::test]
    async fn test_sea_kv_mysql_operations() {
        // Skip if MySQL is not available
        if std::env::var("MYSQL_DSN").is_err() {
            println!("Skipping MySQL tests - MYSQL_DSN not set");
            return;
        }
        
        let dsn = std::env::var("MYSQL_DSN").unwrap_or_else(|_| "mysql://root:password@127.0.0.1:3306/dtm".to_string());
        let kv = SeaKv::mysql(&dsn).await.unwrap();
        
        // Test basic operations
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
        
        kv.put(b"test:key1", b"value1").await.unwrap();
        assert_eq!(kv.get(b"test:key1").await.unwrap(), Some(b"value1".to_vec()));
        
        kv.delete(b"test:key1").await.unwrap();
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sea_kv_postgres_operations() {
        // Skip if PostgreSQL is not available
        if std::env::var("POSTGRES_DSN").is_err() {
            println!("Skipping PostgreSQL tests - POSTGRES_DSN not set");
            return;
        }
        
        let dsn = std::env::var("POSTGRES_DSN").unwrap_or_else(|_| "postgres://postgres:mysecretpassword@127.0.0.1:5432/postgres".to_string());
        let kv = SeaKv::postgres(&dsn).await.unwrap();
        
        // Test basic operations
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
        
        kv.put(b"test:key1", b"value1").await.unwrap();
        assert_eq!(kv.get(b"test:key1").await.unwrap(), Some(b"value1".to_vec()));
        
        kv.delete(b"test:key1").await.unwrap();
        assert!(kv.get(b"test:key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-concurrent-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        // Create multiple transactions concurrently
        let mut handles = Vec::new();
        for i in 0..10 {
            let repo = repo.clone();
            let handle = tokio::spawn(async move {
                let now = chrono::Utc::now().timestamp();
                let txn = GlobalTxn {
                    gid: format!("concurrent-gid-{}", i),
                    mode: "saga".to_string(),
                    status: GlobalStatus::Submitted,
                    payload: format!("payload-{}", i).as_bytes().to_vec(),
                    branches: vec![],
                    updated_unix: now,
                    created_unix: now,
                };
                
                repo.save(&txn).await.unwrap();
                repo.load(&format!("concurrent-gid-{}", i)).await.unwrap()
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }
        
        // Verify all transactions exist
        let list = repo.list(20, 0, None).await.unwrap();
        assert!(list.len() >= 10);
    }

    #[tokio::test]
    async fn test_transaction_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-serialization-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        let now = chrono::Utc::now().timestamp();
        let original_txn = GlobalTxn {
            gid: "serialization-test".to_string(),
            mode: "saga".to_string(),
            status: GlobalStatus::Committing,
            payload: b"complex payload with \x00\x01\x02 bytes".to_vec(),
            branches: vec![
                BranchTxn {
                    branch_id: "branch-1".to_string(),
                    action: "http://example.com/action1?param=value&other=123".to_string(),
                    status: BranchStatus::Succeed,
                },
                BranchTxn {
                    branch_id: "branch-2".to_string(),
                    action: "http://example.com/action2".to_string(),
                    status: BranchStatus::Failed,
                },
            ],
            updated_unix: now,
            created_unix: now - 100,
        };
        
        // Save and load
        repo.save(&original_txn).await.unwrap();
        let loaded_txn = repo.load("serialization-test").await.unwrap().unwrap();
        
        // Verify all fields match
        assert_eq!(original_txn.gid, loaded_txn.gid);
        assert_eq!(original_txn.mode, loaded_txn.mode);
        assert_eq!(original_txn.status, loaded_txn.status);
        assert_eq!(original_txn.payload, loaded_txn.payload);
        assert_eq!(original_txn.branches.len(), loaded_txn.branches.len());
        assert_eq!(original_txn.updated_unix, loaded_txn.updated_unix);
        assert_eq!(original_txn.created_unix, loaded_txn.created_unix);
        
        // Verify branch details
        for (original, loaded) in original_txn.branches.iter().zip(loaded_txn.branches.iter()) {
            assert_eq!(original.branch_id, loaded.branch_id);
            assert_eq!(original.action, loaded.action);
            assert_eq!(original.status, loaded.status);
        }
    }

    #[tokio::test]
    async fn test_large_payload_handling() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-large-payload-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        // Create a large payload (1MB)
        let large_payload = vec![0u8; 1024 * 1024];
        
        let now = chrono::Utc::now().timestamp();
        let txn = GlobalTxn {
            gid: "large-payload-test".to_string(),
            mode: "saga".to_string(),
            status: GlobalStatus::Submitted,
            payload: large_payload.clone(),
            branches: vec![],
            updated_unix: now,
            created_unix: now,
        };
        
        // Save and load large payload
        repo.save(&txn).await.unwrap();
        let loaded = repo.load("large-payload-test").await.unwrap().unwrap();
        
        assert_eq!(loaded.payload.len(), large_payload.len());
        assert_eq!(loaded.payload, large_payload);
    }

    #[tokio::test]
    async fn test_list_pagination() {
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("test-pagination-sled");
        
        let kv = SledKv::open(sled_path.to_str().unwrap()).unwrap();
        let repo = Arc::new(DynTxnRepo::new(Box::new(kv)));
        
        // Create multiple transactions
        for i in 0..5 {
            let now = chrono::Utc::now().timestamp();
            let txn = GlobalTxn {
                gid: format!("pagination-gid-{}", i),
                mode: "saga".to_string(),
                status: if i % 2 == 0 { GlobalStatus::Submitted } else { GlobalStatus::Committed },
                payload: format!("payload-{}", i).as_bytes().to_vec(),
                branches: vec![],
                updated_unix: now,
                created_unix: now,
            };
            repo.save(&txn).await.unwrap();
        }
        
        // Test pagination
        let page1 = repo.list(2, 0, None).await.unwrap();
        assert_eq!(page1.len(), 2);
        
        let page2 = repo.list(2, 2, None).await.unwrap();
        assert_eq!(page2.len(), 2);
        
        let page3 = repo.list(2, 4, None).await.unwrap();
        assert!(page3.len() <= 2);
        
        // Test status filtering
        let submitted = repo.list(10, 0, Some(GlobalStatus::Submitted)).await.unwrap();
        assert!(submitted.len() >= 3); // At least 3 submitted transactions
        
        let committed = repo.list(10, 0, Some(GlobalStatus::Committed)).await.unwrap();
        assert!(committed.len() >= 2); // At least 2 committed transactions
    }
}
