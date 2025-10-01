use crate::test_utils::TestRepo;
use crate::domain::{GlobalStatus, BranchStatus};
use crate::saga::{SagaManager, ExecConfig};
use crate::barrier::{Barrier, BarrierOps};
use crate::storage::KV;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_saga_lifecycle() {
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

        // 1. Start transaction
        let gid = saga.start("integration-test".to_string(), "saga".to_string(), b"test payload".to_vec()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Submitted);
        assert_eq!(tx.branches.len(), 0);

        // 2. Add branches
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "branch2".to_string(), "http://example.com/action2".to_string()).await.unwrap();
        
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 2);
        assert_eq!(tx.branches[0].status, BranchStatus::Prepared);
        assert_eq!(tx.branches[1].status, BranchStatus::Prepared);

        // 2.5. Set branches to succeed
        saga.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "branch2".to_string()).await.unwrap();

        // 3. Submit transaction
        saga.submit(gid.clone()).await.unwrap();
        
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert!(tx.branches.iter().all(|b| b.status == BranchStatus::Succeed));
    }

    #[tokio::test]
    async fn test_saga_with_barrier_integration() {
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

        let gid = saga.start("barrier-test".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Test barrier with saga operations
        let barrier = Arc::new(Barrier::new(TestKV::default())) as Arc<dyn BarrierOps>;

        // Try phase
        let inserted = barrier.insert(&gid, "branch1", crate::domain::BarrierPhase::Try).await.unwrap();
        assert!(inserted);

        // Add branch after barrier
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action".to_string()).await.unwrap();

        // Confirm phase
        let inserted = barrier.insert(&gid, "branch1", crate::domain::BarrierPhase::Confirm).await.unwrap();
        assert!(inserted);

        // Succeed branch
        saga.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].status, BranchStatus::Succeed);
    }

    #[tokio::test]
    async fn test_concurrent_transaction_processing() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 16, // High parallelism
                })
        );

        // Create multiple transactions concurrently
        let mut handles = Vec::new();
        for i in 0..50 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("concurrent-test-{}", i);
                saga.start(gid.clone(), "saga".to_string(), format!("payload-{}", i).as_bytes().to_vec()).await?;
                
                // Add multiple branches
                for j in 0..5 {
                    saga.add_branch(gid.clone(), format!("branch-{}", j), format!("http://example.com/action{}-{}", i, j)).await?;
                }
                
                // Set all branches to succeed
                for j in 0..5 {
                    saga.branch_succeed(gid.clone(), format!("branch-{}", j)).await?;
                }
                
                // Submit transaction
                saga.submit(gid.clone()).await?;
                
                // Verify final state
                let tx = saga.get(&gid).await?;
                assert!(tx.is_some());
                let tx = tx.unwrap();
                assert_eq!(tx.status, GlobalStatus::Committed);
                assert_eq!(tx.branches.len(), 5);
                
                Ok::<(), anyhow::Error>(())
            });
            handles.push(handle);
        }

        // Wait for all transactions to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_transaction_rollback_scenario() {
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

        let gid = saga.start("rollback-test".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Add branches
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "branch2".to_string(), "http://example.com/action2".to_string()).await.unwrap();

        // Manually fail one branch
        saga.branch_fail(gid.clone(), "branch1".to_string()).await.unwrap();

        // Submit transaction (should result in rollback)
        saga.submit(gid.clone()).await.unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        assert!(tx.branches.iter().any(|b| b.status == BranchStatus::Failed));
    }

    #[tokio::test]
    async fn test_mixed_success_failure_branches() {
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

        let gid = saga.start("mixed-test".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Add branches
        saga.add_branch(gid.clone(), "success-branch".to_string(), "http://example.com/success".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "failure-branch".to_string(), "http://example.com/failure".to_string()).await.unwrap();

        // Manually set branch statuses
        saga.branch_succeed(gid.clone(), "success-branch".to_string()).await.unwrap();
        saga.branch_fail(gid.clone(), "failure-branch".to_string()).await.unwrap();

        // Submit transaction
        saga.submit(gid.clone()).await.unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        let success_branch = tx.branches.iter().find(|b| b.branch_id == "success-branch").unwrap();
        let failure_branch = tx.branches.iter().find(|b| b.branch_id == "failure-branch").unwrap();
        
        assert_eq!(success_branch.status, BranchStatus::Succeed);
        assert_eq!(failure_branch.status, BranchStatus::Failed);
    }

    #[tokio::test]
    async fn test_transaction_listing_and_filtering() {
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

        // Create transactions with different statuses
        let submitted_gid = saga.start("submitted-tx".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        
        let committed_gid = saga.start("committed-tx".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        saga.submit(committed_gid.clone()).await.unwrap();
        
        let aborted_gid = saga.start("aborted-tx".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        saga.abort(aborted_gid.clone()).await.unwrap();

        // Test listing all transactions
        let all_txs = saga.list(10, 0, None).await.unwrap();
        assert!(all_txs.len() >= 3);

        // Test filtering by status
        let submitted_txs = saga.list(10, 0, Some(GlobalStatus::Submitted)).await.unwrap();
        assert!(submitted_txs.iter().any(|tx| tx.gid == submitted_gid));

        let committed_txs = saga.list(10, 0, Some(GlobalStatus::Committed)).await.unwrap();
        assert!(committed_txs.iter().any(|tx| tx.gid == committed_gid));

        let aborted_txs = saga.list(10, 0, Some(GlobalStatus::Aborted)).await.unwrap();
        assert!(aborted_txs.iter().any(|tx| tx.gid == aborted_gid));
    }

    #[tokio::test]
    async fn test_pagination_functionality() {
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

        // Create 20 transactions
        let mut gids = Vec::new();
        for i in 0..20 {
            let gid = saga.start(format!("pagination-test-{}", i), "saga".to_string(), b"payload".to_vec()).await.unwrap();
            gids.push(gid);
        }

        // Test pagination
        let page1 = saga.list(5, 0, None).await.unwrap();
        assert_eq!(page1.len(), 5);

        let page2 = saga.list(5, 5, None).await.unwrap();
        assert_eq!(page2.len(), 5);

        let page3 = saga.list(5, 10, None).await.unwrap();
        assert_eq!(page3.len(), 5);

        let page4 = saga.list(5, 15, None).await.unwrap();
        assert_eq!(page4.len(), 5);

        // Verify no overlap between pages
        let all_ids: std::collections::HashSet<String> = page1.iter().chain(page2.iter()).chain(page3.iter()).chain(page4.iter())
            .map(|tx| tx.gid.clone())
            .collect();
        assert_eq!(all_ids.len(), 20); // All unique
    }

    #[tokio::test]
    async fn test_storage_persistence() {
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

        // Create and modify transaction
        let gid = saga.start("persistence-test".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();
        saga.submit(gid.clone()).await.unwrap();

        // Verify persistence by retrieving transaction
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 1);
        assert_eq!(tx.branches[0].status, BranchStatus::Succeed);
        assert_eq!(tx.branches[0].branch_id, "branch1");
    }

    #[tokio::test]
    async fn test_error_recovery_scenarios() {
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

        let gid = saga.start("recovery-test".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();

        // Simulate partial failure scenario
        saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action1".to_string()).await.unwrap();
        saga.add_branch(gid.clone(), "branch2".to_string(), "http://example.com/action2".to_string()).await.unwrap();

        // One branch succeeds, one fails
        saga.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();
        saga.branch_fail(gid.clone(), "branch2".to_string()).await.unwrap();

        // Submit should handle mixed results
        saga.submit(gid.clone()).await.unwrap();

        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        let branch1 = tx.branches.iter().find(|b| b.branch_id == "branch1").unwrap();
        let branch2 = tx.branches.iter().find(|b| b.branch_id == "branch2").unwrap();
        
        assert_eq!(branch1.status, BranchStatus::Succeed);
        assert_eq!(branch2.status, BranchStatus::Failed);
    }

    #[tokio::test]
    async fn test_high_throughput_scenario() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 32, // High parallelism
                })
        );

        let start = std::time::Instant::now();
        let mut handles = Vec::new();

        // Create 200 transactions with 3 branches each
        for i in 0..200 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("throughput-test-{}", i);
                saga.start(gid.clone(), "saga".to_string(), b"payload".to_vec()).await?;
                
                // Add 3 branches per transaction
                for j in 0..3 {
                    saga.add_branch(gid.clone(), format!("branch-{}", j), format!("http://example.com/action{}-{}", i, j)).await?;
                }
                
                // Submit transaction
                saga.submit(gid.clone()).await?;
                
                Ok::<(), anyhow::Error>(())
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        let duration = start.elapsed();
        println!("Processed 200 transactions with 3 branches each in {:?}", duration);
        assert!(duration < std::time::Duration::from_secs(10));

        // Verify all transactions were processed
        let list = saga.list(1000, 0, None).await.unwrap();
        assert!(list.len() >= 200);
    }
}

// Helper struct for testing
#[derive(Clone, Default)]
struct TestKV {
    inner: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<Vec<u8>, Vec<u8>>>>,
}

#[async_trait::async_trait]
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
