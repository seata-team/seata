use crate::test_utils::TestRepo;
use crate::domain::BranchStatus;
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_transaction_creation() {
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

        let start = Instant::now();
        let mut handles = Vec::new();

        // Create 100 transactions concurrently
        for i in 0..100 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("perf-test-{}", i);
                let payload = format!("payload-{}", i).as_bytes().to_vec();
                saga.start(gid, "saga".to_string(), payload).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Created 100 transactions in {:?}", duration);
        assert!(duration < Duration::from_secs(5)); // Should complete quickly
    }

    #[tokio::test]
    async fn test_large_payload_performance() {
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

        // Test with different payload sizes
        let payload_sizes = vec![1024, 10240, 102400, 1024000]; // 1KB, 10KB, 100KB, 1MB

        for size in payload_sizes {
            let payload = vec![0u8; size];
            let start = Instant::now();
            
            let gid = saga.start(
                format!("large-payload-{}", size),
                "saga".to_string(),
                payload
            ).await.unwrap();
            
            let duration = start.elapsed();
            println!("Created transaction with {} bytes payload in {:?}", size, duration);
            
            // Verify the transaction was created
            let tx = saga.get(&gid).await.unwrap().unwrap();
            assert_eq!(tx.payload.len(), size);
        }
    }

    #[tokio::test]
    async fn test_branch_parallelism() {
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

        let gid = saga.start(
            "parallelism-test".to_string(),
            "saga".to_string(),
            b"test payload".to_vec()
        ).await.unwrap();

        let start = Instant::now();
        let mut handles = Vec::new();

        // Add 50 branches concurrently
        for i in 0..50 {
            let saga = saga.clone();
            let gid = gid.clone();
            let handle = tokio::spawn(async move {
                saga.add_branch(
                    gid,
                    format!("branch-{}", i),
                    format!("http://example.com/action{}", i)
                ).await
            });
            handles.push(handle);
        }

        // Wait for all branches to be added
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Added 50 branches in {:?}", duration);
        assert!(duration < Duration::from_secs(2)); // Should complete quickly

        // Verify all branches were added
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 50);
    }

    #[tokio::test]
    async fn test_memory_usage_with_many_transactions() {
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

        let start = Instant::now();
        let mut gids = Vec::new();

        // Create 1000 transactions
        for i in 0..1000 {
            let gid = saga.start(
                format!("memory-test-{}", i),
                "saga".to_string(),
                format!("payload-{}", i).as_bytes().to_vec()
            ).await.unwrap();
            gids.push(gid);
        }

        let duration = start.elapsed();
        println!("Created 1000 transactions in {:?}", duration);

        // Verify all transactions exist
        for gid in gids {
            let tx = saga.get(&gid).await.unwrap();
            assert!(tx.is_some());
        }
    }

    #[tokio::test]
    async fn test_list_performance() {
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

        // Create 500 transactions
        for i in 0..500 {
            saga.start(
                format!("list-test-{}", i),
                "saga".to_string(),
                format!("payload-{}", i).as_bytes().to_vec()
            ).await.unwrap();
        }

        // Test list performance with different limits
        let limits = vec![10, 50, 100, 500];
        
        for limit in limits {
            let start = Instant::now();
            let list = saga.list(limit, 0, None).await.unwrap();
            let duration = start.elapsed();
            
            println!("Listed {} transactions (limit {}) in {:?}", list.len(), limit, duration);
            assert!(list.len() <= limit);
        }
    }

    #[tokio::test]
    async fn test_concurrent_read_write() {
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

        let start = Instant::now();
        let mut handles = Vec::new();

        // Mix of read and write operations
        for i in 0..100 {
            let saga = saga.clone();
            let handle = if i % 2 == 0 {
                // Write operation
                tokio::spawn(async move {
                    let gid = format!("rw-test-{}", i);
                    saga.start(gid, "saga".to_string(), format!("payload-{}", i).as_bytes().to_vec()).await
                })
            } else {
                // Read operation
                let gid = format!("rw-test-{}", i - 1);
                tokio::spawn(async move {
                    saga.get(&gid).await.map(|_| "read".to_string())
                })
            };
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Completed 100 mixed read/write operations in {:?}", duration);
        assert!(duration < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_branch_status_updates() {
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

        let gid = saga.start(
            "branch-status-test".to_string(),
            "saga".to_string(),
            b"test payload".to_vec()
        ).await.unwrap();

        // Add 20 branches
        for i in 0..20 {
            saga.add_branch(
                gid.clone(),
                format!("branch-{}", i),
                format!("http://example.com/action{}", i)
            ).await.unwrap();
        }

        let start = Instant::now();
        let mut handles = Vec::new();

        // Update branch statuses concurrently
        for i in 0..20 {
            let saga = saga.clone();
            let gid = gid.clone();
            let handle = if i % 2 == 0 {
                tokio::spawn(async move {
                    saga.branch_succeed(gid, format!("branch-{}", i)).await
                })
            } else {
                tokio::spawn(async move {
                    saga.branch_fail(gid, format!("branch-{}", i)).await
                })
            };
            handles.push(handle);
        }

        // Wait for all updates to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Updated 20 branch statuses in {:?}", duration);

        // Verify the transaction state
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 20);
        
        let succeed_count = tx.branches.iter().filter(|b| b.status == BranchStatus::Succeed).count();
        let fail_count = tx.branches.iter().filter(|b| b.status == BranchStatus::Failed).count();
        
        assert_eq!(succeed_count, 10);
        assert_eq!(fail_count, 10);
    }

    #[tokio::test]
    async fn test_transaction_lifecycle_performance() {
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

        let start = Instant::now();
        let mut handles = Vec::new();

        // Complete transaction lifecycle for 50 transactions
        for i in 0..50 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("lifecycle-test-{}", i);
                
                // Start transaction
                saga.start(gid.clone(), "saga".to_string(), format!("payload-{}", i).as_bytes().to_vec()).await?;
                
                // Add branch
                saga.add_branch(gid.clone(), "branch1".to_string(), "http://example.com/action".to_string()).await?;
                
                // Submit transaction
                saga.submit(gid.clone()).await?;
                
                // Get transaction
                let tx = saga.get(&gid).await?;
                assert!(tx.is_some());
                
                Ok::<(), anyhow::Error>(())
            });
            handles.push(handle);
        }

        // Wait for all lifecycles to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Completed 50 transaction lifecycles in {:?}", duration);
        assert!(duration < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
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

        // Create transactions with varying payload sizes
        let payload_sizes = vec![100, 1000, 10000, 100000];
        let mut gids = Vec::new();

        for size in &payload_sizes {
            for i in 0..10 {
                let payload = vec![0u8; *size];
                let gid = saga.start(
                    format!("memory-eff-test-{}-{}", size, i),
                    "saga".to_string(),
                    payload
                ).await.unwrap();
                gids.push(gid);
            }
        }

        // Verify all transactions exist and have correct payload sizes
        for (i, gid) in gids.iter().enumerate() {
            let tx = saga.get(gid).await.unwrap().unwrap();
            let expected_size = payload_sizes[i / 10];
            assert_eq!(tx.payload.len(), expected_size);
        }
    }

    #[tokio::test]
    async fn test_concurrent_submit_performance() {
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

        // Create 20 transactions with branches
        let mut gids = Vec::new();
        for i in 0..20 {
            let gid = saga.start(
                format!("submit-perf-test-{}", i),
                "saga".to_string(),
                format!("payload-{}", i).as_bytes().to_vec()
            ).await.unwrap();
            
            // Add 3 branches to each transaction
            for j in 0..3 {
                saga.add_branch(
                    gid.clone(),
                    format!("branch-{}", j),
                    format!("http://example.com/action{}-{}", i, j)
                ).await.unwrap();
            }
            
            gids.push(gid);
        }

        let start = Instant::now();
        let mut handles = Vec::new();

        // Submit all transactions concurrently
        for gid in gids {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                saga.submit(gid).await
            });
            handles.push(handle);
        }

        // Wait for all submissions to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        println!("Submitted 20 transactions with 3 branches each in {:?}", duration);
        // Allow more time for HTTP timeouts (branches will fail but that's expected)
        assert!(duration < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_large_branch_count_performance() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 16, // High parallelism for many branches
                })
        );

        let gid = saga.start(
            "large-branch-test".to_string(),
            "saga".to_string(),
            b"test payload".to_vec()
        ).await.unwrap();

        let start = Instant::now();

        // Add 100 branches
        for i in 0..100 {
            saga.add_branch(
                gid.clone(),
                format!("branch-{}", i),
                format!("http://example.com/action{}", i)
            ).await.unwrap();
        }

        let duration = start.elapsed();
        println!("Added 100 branches in {:?}", duration);

        // Verify all branches were added
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.branches.len(), 100);
    }
}
