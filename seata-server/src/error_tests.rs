use crate::test_utils::TestRepo;
use crate::domain::GlobalStatus;
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_gid_handling() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        // Test with empty GID
        let gid = saga.start("".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        assert!(!gid.is_empty()); // Should generate a UUID

        // Test with very long GID
        let long_gid = "a".repeat(1000);
        let gid = saga.start(long_gid.clone(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        assert_eq!(gid, long_gid);

        // Test with special characters in GID
        let special_gid = "test-gid-with-special-chars!@#$%^&*()";
        let gid = saga.start(special_gid.to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        assert_eq!(gid, special_gid);
    }

    #[tokio::test]
    async fn test_invalid_payload_handling() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        // Test with empty payload
        let gid = saga.start("test-gid".to_string(), "saga".to_string(), vec![]).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert!(tx.payload.is_empty());

        // Test with null bytes in payload
        let payload_with_nulls = vec![0, 1, 2, 0, 3, 4, 0];
        let gid = saga.start("test-gid-nulls".to_string(), "saga".to_string(), payload_with_nulls.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.payload, payload_with_nulls);

        // Test with very large payload
        let large_payload = vec![0u8; 1024 * 1024]; // 1MB
        let gid = saga.start("test-gid-large".to_string(), "saga".to_string(), large_payload.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.payload.len(), large_payload.len());
    }

    #[tokio::test]
    async fn test_invalid_branch_handling() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let gid = saga.start("test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Test with empty branch_id
        saga.add_branch(gid.clone(), "".to_string(), "http://example.com/action".to_string()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].branch_id, "");

        // Test with very long branch_id
        let long_branch_id = "b".repeat(1000);
        saga.add_branch(gid.clone(), long_branch_id.clone(), "http://example.com/action".to_string()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 2);
        assert_eq!(tx.branches[1].branch_id, long_branch_id);

        // Test with invalid URL
        saga.add_branch(gid.clone(), "branch-invalid-url".to_string(), "not-a-url".to_string()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 3);
        assert_eq!(tx.branches[2].action, "not-a-url");
    }

    #[tokio::test]
    async fn test_duplicate_branch_handling() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let gid = saga.start("test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Add the same branch multiple times
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        // Should only have one branch (duplicates are ignored)
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].branch_id, "branch1");
    }

    #[tokio::test]
    async fn test_nonexistent_transaction_operations() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let nonexistent_gid = "nonexistent-gid";

        // Test operations on non-existent transaction
        saga.submit(nonexistent_gid.to_string()).await.unwrap(); // Should not error
        saga.abort(nonexistent_gid.to_string()).await.unwrap(); // Should not error
        saga.add_branch(nonexistent_gid.to_string(), "branch1".to_string(), "http://example.com/action".to_string()).await.unwrap(); // Should not error
        saga.branch_succeed(nonexistent_gid.to_string(), "branch1".to_string()).await.unwrap(); // Should not error
        saga.branch_fail(nonexistent_gid.to_string(), "branch1".to_string()).await.unwrap(); // Should not error

        // Test getting non-existent transaction
        let tx = saga.get(nonexistent_gid).await.unwrap();
        assert!(tx.is_none());
    }

    #[tokio::test]
    async fn test_nonexistent_branch_operations() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let gid = saga.start("test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Test operations on non-existent branch
        saga.branch_succeed(gid.clone(), "nonexistent-branch".to_string()).await.unwrap(); // Should not error
        saga.branch_fail(gid.clone(), "nonexistent-branch".to_string()).await.unwrap(); // Should not error

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 0); // No branches should be added
    }

    #[tokio::test]
    async fn test_concurrent_duplicate_operations() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let gid = saga.start("test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Add the same branch concurrently
        let saga1 = saga.clone();
        let saga2 = saga.clone();
        let saga3 = saga.clone();
        let gid1 = gid.clone();
        let gid2 = gid.clone();
        let gid3 = gid.clone();

        let handle1 = tokio::spawn(async move {
            saga1.add_branch(gid1, "branch1".to_string(), "http://example.com/action1".to_string()).await
        });
        let handle2 = tokio::spawn(async move {
            saga2.add_branch(gid2, "branch1".to_string(), "http://example.com/action2".to_string()).await
        });
        let handle3 = tokio::spawn(async move {
            saga3.add_branch(gid3, "branch1".to_string(), "http://example.com/action3".to_string()).await
        });

        // Wait for all operations to complete
        handle1.await.unwrap().unwrap();
        handle2.await.unwrap().unwrap();
        handle3.await.unwrap().unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        // Should only have one branch (duplicates are ignored)
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].branch_id, "branch1");
    }

    #[tokio::test]
    async fn test_malformed_data_handling() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        // Test with Unicode characters
        let unicode_gid = "测试-事务-标识符";
        let unicode_payload = "测试负载数据".as_bytes().to_vec();
        let gid = saga.start(unicode_gid.to_string(), "saga".to_string(), unicode_payload.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.payload, unicode_payload);

        // Test with special characters in branch_id
        let special_branch_id = "branch-with-特殊字符-!@#$%";
        saga.add_branch(gid.clone(), special_branch_id.to_string(), "http://example.com/action".to_string()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].branch_id, special_branch_id);
    }

    #[tokio::test]
    async fn test_extreme_values() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        // Test with large string lengths (reduced for faster execution)
        let max_gid = "a".repeat(1000); // Reduced from 10000
        let max_payload = vec![0u8; 1024 * 1024]; // 1MB instead of 10MB
        let gid = saga.start(max_gid.clone(), "saga".to_string(), max_payload.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.gid, max_gid);
        assert_eq!(tx.payload.len(), max_payload.len());

        // Test with many branches (reduced count for faster execution)
        for i in 0..100 { // Reduced from 1000
            saga.add_branch(gid.clone(), format!("branch-{}", i), format!("http://example.com/action{}", i)).await.unwrap();
        }

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 100);
    }

    #[tokio::test]
    async fn test_race_conditions() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        let gid = saga.start("race-test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Concurrent submit and abort
        let saga1 = saga.clone();
        let saga2 = saga.clone();
        let gid1 = gid.clone();
        let gid2 = gid.clone();

        let handle1 = tokio::spawn(async move {
            saga1.submit(gid1).await
        });
        let handle2 = tokio::spawn(async move {
            saga2.abort(gid2).await
        });

        // Both should complete without error
        handle1.await.unwrap().unwrap();
        handle2.await.unwrap().unwrap();

        // The transaction should be in a consistent state
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert!(matches!(tx.status, GlobalStatus::Aborted | GlobalStatus::Committed));
    }

    #[tokio::test]
    async fn test_memory_pressure() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 8,
                })
        );

        // Create many transactions to test memory handling
        let mut gids = Vec::new();
        for i in 0..100 {
            let gid = saga.start(
                format!("memory-test-{}", i),
                "saga".to_string(),
                vec![0u8; 1024 * 100] // 100KB per transaction
            ).await.unwrap();
            gids.push(gid);
        }

        // Verify all transactions exist
        for gid in &gids {
            let tx = saga.get(gid).await.unwrap();
            assert!(tx.is_some());
        }

        // Test list with large number of transactions
        let list = saga.list(1000, 0, None).await.unwrap();
        assert!(list.len() >= 100);
    }
}
