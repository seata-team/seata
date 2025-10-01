use crate::test_utils::TestRepo;
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn benchmark_transaction_creation() {
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

        let iterations = 1000;
        let start = Instant::now();

        for i in 0..iterations {
            saga.start(
                format!("benchmark-{}", i),
                "saga".to_string(),
                format!("payload-{}", i).as_bytes().to_vec()
            ).await.unwrap();
        }

        let duration = start.elapsed();
        let tps = iterations as f64 / duration.as_secs_f64();
        
        println!("Transaction creation benchmark:");
        println!("  Transactions: {}", iterations);
        println!("  Duration: {:?}", duration);
        println!("  TPS: {:.2}", tps);
        
        assert!(tps > 100.0); // Should handle at least 100 TPS
    }

    #[tokio::test]
    async fn benchmark_branch_operations() {
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

        let gid = saga.start("branch-benchmark".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        let iterations = 1000;
        let start = Instant::now();

        for i in 0..iterations {
            saga.add_branch(
                gid.clone(),
                format!("branch-{}", i),
                format!("http://example.com/action{}", i)
            ).await.unwrap();
        }

        let duration = start.elapsed();
        let ops_per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("Branch operations benchmark:");
        println!("  Operations: {}", iterations);
        println!("  Duration: {:?}", duration);
        println!("  Ops/sec: {:.2}", ops_per_sec);
        
        assert!(ops_per_sec > 50.0); // Should handle at least 50 ops/sec
    }

    #[tokio::test]
    async fn benchmark_concurrent_transactions() {
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

        let concurrent_tasks = 100;
        let transactions_per_task = 10;
        let start = Instant::now();

        let mut handles = Vec::new();
        for task_id in 0..concurrent_tasks {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                for i in 0..transactions_per_task {
                    let gid = format!("concurrent-{}-{}", task_id, i);
                    saga.start(gid, "saga".to_string(), b"payload".to_vec()).await.unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();
        let total_transactions = concurrent_tasks * transactions_per_task;
        let tps = total_transactions as f64 / duration.as_secs_f64();
        
        println!("Concurrent transactions benchmark:");
        println!("  Concurrent tasks: {}", concurrent_tasks);
        println!("  Transactions per task: {}", transactions_per_task);
        println!("  Total transactions: {}", total_transactions);
        println!("  Duration: {:?}", duration);
        println!("  TPS: {:.2}", tps);
        
        assert!(tps > 200.0); // Should handle at least 200 TPS
    }

    #[tokio::test]
    async fn benchmark_large_payload_handling() {
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

        let payload_sizes = vec![1024, 10240, 102400, 1024000]; // 1KB, 10KB, 100KB, 1MB
        let iterations = 10;

        for size in payload_sizes {
            let payload = vec![0u8; size];
            let start = Instant::now();

            for i in 0..iterations {
                saga.start(
                    format!("large-payload-{}-{}", size, i),
                    "saga".to_string(),
                    payload.clone()
                ).await.unwrap();
            }

            let duration = start.elapsed();
            let throughput_mbps = (size * iterations) as f64 / duration.as_secs_f64() / 1024.0 / 1024.0;
            
            println!("Large payload benchmark ({} bytes):", size);
            println!("  Iterations: {}", iterations);
            println!("  Duration: {:?}", duration);
            println!("  Throughput: {:.2} MB/s", throughput_mbps);
        }
    }

    #[tokio::test]
    async fn benchmark_memory_usage() {
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

        let transaction_counts = vec![100, 500, 1000, 2000];
        
        for count in transaction_counts {
            let start = Instant::now();
            let mut gids = Vec::new();

            for i in 0..count {
                let gid = saga.start(
                    format!("memory-test-{}", i),
                    "saga".to_string(),
                    format!("payload-{}", i).as_bytes().to_vec()
                ).await.unwrap();
                gids.push(gid);
            }

            let duration = start.elapsed();
            let tps = count as f64 / duration.as_secs_f64();
            
            println!("Memory usage benchmark ({} transactions):", count);
            println!("  Duration: {:?}", duration);
            println!("  TPS: {:.2}", tps);

            // Verify all transactions exist
            for gid in &gids {
                let tx = saga.get(gid).await.unwrap();
                assert!(tx.is_some());
            }
        }
    }

    #[tokio::test]
    async fn benchmark_list_operations() {
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

        // Create test data
        for i in 0..1000 {
            saga.start(
                format!("list-benchmark-{}", i),
                "saga".to_string(),
                b"payload".to_vec()
            ).await.unwrap();
        }

        let list_operations = vec![10, 50, 100, 500];
        
        for limit in list_operations {
            let start = Instant::now();
            let iterations = 100;

            for _ in 0..iterations {
                saga.list(limit, 0, None).await.unwrap();
            }

            let duration = start.elapsed();
            let ops_per_sec = iterations as f64 / duration.as_secs_f64();
            
            println!("List operations benchmark (limit {}):", limit);
            println!("  Operations: {}", iterations);
            println!("  Duration: {:?}", duration);
            println!("  Ops/sec: {:.2}", ops_per_sec);
        }
    }

    #[tokio::test]
    async fn benchmark_mixed_workload() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 16,
                })
        );

        let start = Instant::now();
        let mut handles = Vec::new();

        // Mix of operations: 50% creates, 30% reads, 20% updates
        for i in 0..100 {
            let saga = saga.clone();
            let operation_type = i % 10;
            
            let handle = tokio::spawn(async move {
                match operation_type {
                    0..=4 => {
                        // Create transaction
                        saga.start(
                            format!("mixed-workload-{}", i),
                            "saga".to_string(),
                            b"payload".to_vec()
                        ).await
                    }
                    5..=7 => {
                        // Read transaction
                        saga.get(&format!("mixed-workload-{}", i)).await.map(|_| "read".to_string())
                    }
                    _ => {
                        // Update transaction (add branch)
                        let gid = format!("mixed-workload-{}", i);
                        saga.add_branch(gid, "branch".to_string(), "http://example.com/action".to_string()).await.map(|_| "update".to_string())
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let _ = handle.await.unwrap();
        }

        let duration = start.elapsed();
        let ops_per_sec = 100.0 / duration.as_secs_f64();
        
        println!("Mixed workload benchmark:");
        println!("  Operations: 100 (50% create, 30% read, 20% update)");
        println!("  Duration: {:?}", duration);
        println!("  Ops/sec: {:.2}", ops_per_sec);
        
        assert!(ops_per_sec > 10.0); // Should handle mixed workload
    }

    #[tokio::test]
    async fn benchmark_stress_test() {
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

        let start = Instant::now();
        let mut handles = Vec::new();

        // Stress test: 500 concurrent transactions with 5 branches each
        for i in 0..500 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("stress-test-{}", i);
                saga.start(gid.clone(), "saga".to_string(), b"payload".to_vec()).await?;
                
                // Add 5 branches
                for j in 0..5 {
                    saga.add_branch(
                        gid.clone(),
                        format!("branch-{}", j),
                        format!("http://example.com/action{}-{}", i, j)
                    ).await?;
                }
                
                // Submit transaction
                saga.submit(gid).await?;
                
                Ok::<(), anyhow::Error>(())
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }

        let duration = start.elapsed();
        let tps = success_count as f64 / duration.as_secs_f64();
        
        println!("Stress test benchmark:");
        println!("  Concurrent transactions: 500");
        println!("  Branches per transaction: 5");
        println!("  Successful transactions: {}", success_count);
        println!("  Duration: {:?}", duration);
        println!("  TPS: {:.2}", tps);
        
        assert!(success_count >= 400); // At least 80% should succeed
        assert!(tps > 50.0); // Should maintain reasonable throughput
    }

    #[tokio::test]
    async fn benchmark_latency_measurement() {
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

        let iterations = 1000;
        let mut latencies = Vec::new();

        for i in 0..iterations {
            let start = Instant::now();
            
            saga.start(
                format!("latency-test-{}", i),
                "saga".to_string(),
                b"payload".to_vec()
            ).await.unwrap();
            
            let latency = start.elapsed();
            latencies.push(latency);
        }

        // Calculate statistics
        latencies.sort();
        let min = latencies[0];
        let max = latencies[latencies.len() - 1];
        let median = latencies[latencies.len() / 2];
        let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
        let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];
        
        let avg_duration = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        
        println!("Latency benchmark ({} operations):", iterations);
        println!("  Min: {:?}", min);
        println!("  Max: {:?}", max);
        println!("  Median: {:?}", median);
        println!("  Average: {:?}", avg_duration);
        println!("  P95: {:?}", p95);
        println!("  P99: {:?}", p99);
        
        // Assert reasonable latency bounds
        assert!(median < Duration::from_millis(10));
        assert!(p95 < Duration::from_millis(50));
        assert!(p99 < Duration::from_millis(100));
    }
}
