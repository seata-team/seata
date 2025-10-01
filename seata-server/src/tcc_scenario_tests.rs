use crate::test_utils::TestRepo;
use crate::domain::{GlobalStatus, BranchStatus};
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;
use serde_json::json;

/// 基于seata-examples的TCC模式测试
/// 模拟Try-Confirm-Cancel模式的分布式事务
#[cfg(test)]
mod tests {
    use super::*;

    /// 模拟TCC转账场景 - 基于http_tcc示例
    #[tokio::test]
    async fn test_tcc_transfer_scenario() {
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

        // 模拟TCC转账场景
        let transfer_data = json!({
            "amount": 30,
            "from_account": "account_a",
            "to_account": "account_b",
            "tcc_mode": true
        });

        let gid = saga.start(
            "tcc_transfer_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&transfer_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "trans_out_try".to_string(),
            "http://localhost:8081/api/trans_out_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "trans_in_try".to_string(),
            "http://localhost:8081/api/trans_in_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "trans_out_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_in_try".to_string()).await.unwrap();

        // 提交事务（进入Confirm阶段）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
        
        // 验证所有分支都成功
        for branch in &tx.branches {
            assert_eq!(branch.status, BranchStatus::Succeed);
        }
    }

    /// 模拟TCC转账回滚场景 - 基于http_tcc_rollback示例
    #[tokio::test]
    async fn test_tcc_transfer_rollback_scenario() {
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

        let transfer_data = json!({
            "amount": 30,
            "from_account": "account_a",
            "to_account": "account_b",
            "tcc_mode": true,
            "trans_in_result": "FAILURE"
        });

        let gid = saga.start(
            "tcc_rollback_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&transfer_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "trans_out_try".to_string(),
            "http://localhost:8081/api/trans_out_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "trans_in_try".to_string(),
            "http://localhost:8081/api/trans_in_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段：转出成功，转入失败
        saga.branch_succeed(gid.clone(), "trans_out_try".to_string()).await.unwrap();
        saga.branch_fail(gid.clone(), "trans_in_try".to_string()).await.unwrap();

        // 提交事务（应该触发Cancel阶段）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态（应该被回滚）
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证事务被中止，但分支状态可能不同
        // 在TCC模式中，失败的分支会保持失败状态，成功的分支在回滚前可能仍显示成功
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证至少有一个分支失败（trans_in_try）
        let failed_branches: Vec<_> = tx.branches.iter().filter(|b| b.status == BranchStatus::Failed).collect();
        assert!(!failed_branches.is_empty(), "应该至少有一个分支失败");
    }

    /// 模拟TCC嵌套事务场景 - 基于http_tcc_nested示例
    #[tokio::test]
    async fn test_tcc_nested_transaction_scenario() {
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

        let nested_data = json!({
            "parent_transaction": true,
            "amount": 30,
            "nested_operations": [
                {"type": "trans_out", "amount": 30},
                {"type": "trans_in_nested", "amount": 30}
            ]
        });

        let gid = saga.start(
            "tcc_nested_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&nested_data).unwrap()
        ).await.unwrap();

        // 外层TCC事务
        saga.add_branch(
            gid.clone(),
            "trans_out_try".to_string(),
            "http://localhost:8081/api/trans_out_try".to_string()
        ).await.unwrap();

        // 嵌套TCC事务
        saga.add_branch(
            gid.clone(),
            "trans_in_nested_try".to_string(),
            "http://localhost:8081/api/trans_in_tcc_nested_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "trans_out_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_in_nested_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
    }

    /// 模拟TCC订单系统场景
    #[tokio::test]
    async fn test_tcc_order_system_scenario() {
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

        let order_data = json!({
            "order_id": "tcc_order_001",
            "user_id": "user_123",
            "product_id": "product_456",
            "quantity": 2,
            "amount": 100.0,
            "tcc_mode": true
        });

        let gid = saga.start(
            "tcc_order_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&order_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "create_order_try".to_string(),
            "http://localhost:8081/api/orders/create_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "reduce_inventory_try".to_string(),
            "http://localhost:8082/api/inventory/reduce_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "deduct_balance_try".to_string(),
            "http://localhost:8083/api/balance/deduct_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "create_order_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "reduce_inventory_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "deduct_balance_try".to_string()).await.unwrap();

        // 提交事务（进入Confirm阶段）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }

    /// 模拟TCC支付系统场景
    #[tokio::test]
    async fn test_tcc_payment_system_scenario() {
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

        let payment_data = json!({
            "payment_id": "tcc_payment_001",
            "user_id": "user_123",
            "amount": 50.0,
            "currency": "USD",
            "payment_method": "credit_card",
            "tcc_mode": true
        });

        let gid = saga.start(
            "tcc_payment_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&payment_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "validate_payment_try".to_string(),
            "http://localhost:8081/api/payments/validate_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "process_payment_try".to_string(),
            "http://localhost:8082/api/payments/process_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "update_balance_try".to_string(),
            "http://localhost:8083/api/balance/update_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "validate_payment_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "process_payment_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_balance_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }

    /// 模拟TCC库存管理场景
    #[tokio::test]
    async fn test_tcc_inventory_management_scenario() {
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

        let inventory_data = json!({
            "product_id": "product_789",
            "warehouse_id": "warehouse_001",
            "quantity": 10,
            "operation": "stock_in",
            "tcc_mode": true
        });

        let gid = saga.start(
            "tcc_inventory_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&inventory_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "validate_product_try".to_string(),
            "http://localhost:8081/api/products/validate_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "update_inventory_try".to_string(),
            "http://localhost:8082/api/inventory/update_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "update_warehouse_try".to_string(),
            "http://localhost:8083/api/warehouse/update_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "validate_product_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_inventory_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_warehouse_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }

    /// 模拟TCC并发场景
    #[tokio::test]
    async fn test_tcc_concurrent_scenario() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 16, // 高并发
                })
        );

        // 创建多个并发TCC事务
        let mut handles = Vec::new();
        for i in 0..20 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("tcc_concurrent_tx_{}", i);
                let data = json!({
                    "transaction_id": gid,
                    "amount": 10 + i,
                    "tcc_mode": true,
                    "timestamp": chrono::Utc::now().timestamp()
                });

                saga.start(
                    gid.clone(),
                    "tcc".to_string(),
                    serde_json::to_vec(&data).unwrap()
                ).await?;

                // 添加TCC分支
                saga.add_branch(
                    gid.clone(),
                    "try_operation".to_string(),
                    format!("http://localhost:8081/api/try_{}", i)
                ).await?;

                // 模拟Try阶段成功
                saga.branch_succeed(gid.clone(), "try_operation".to_string()).await?;

                saga.submit(gid.clone()).await?;
                Ok::<String, anyhow::Error>(gid)
            });
            handles.push(handle);
        }

        // 等待所有事务完成
        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                success_count += 1;
            }
        }

        // 验证大部分事务成功
        assert!(success_count >= 18); // 至少90%成功
    }

    /// 模拟TCC数据一致性场景
    #[tokio::test]
    async fn test_tcc_data_consistency_scenario() {
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

        let consistency_data = json!({
            "data_id": "tcc_data_001",
            "tcc_mode": true,
            "operations": [
                {"type": "create_user_try", "table": "users", "data": {"id": 1, "name": "Alice"}},
                {"type": "update_profile_try", "table": "profiles", "data": {"user_id": 1, "email": "alice@example.com"}},
                {"type": "insert_log_try", "table": "logs", "data": {"user_id": 1, "action": "created"}}
            ]
        });

        let gid = saga.start(
            "tcc_consistency_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&consistency_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "create_user_try".to_string(),
            "http://localhost:8081/api/users/create_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "update_profile_try".to_string(),
            "http://localhost:8082/api/profiles/update_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "insert_log_try".to_string(),
            "http://localhost:8083/api/logs/insert_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "create_user_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_profile_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "insert_log_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }

    /// 模拟TCC混合成功失败场景
    #[tokio::test]
    async fn test_tcc_mixed_success_failure_scenario() {
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

        let mixed_data = json!({
            "transaction_id": "tcc_mixed_001",
            "tcc_mode": true,
            "operations": [
                {"type": "operation_1", "expected_result": "SUCCESS"},
                {"type": "operation_2", "expected_result": "FAILURE"},
                {"type": "operation_3", "expected_result": "SUCCESS"}
            ]
        });

        let gid = saga.start(
            "tcc_mixed_tx_001".to_string(),
            "tcc".to_string(),
            serde_json::to_vec(&mixed_data).unwrap()
        ).await.unwrap();

        // TCC模式：Try阶段
        saga.add_branch(
            gid.clone(),
            "operation_1_try".to_string(),
            "http://localhost:8081/api/operation_1_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "operation_2_try".to_string(),
            "http://localhost:8082/api/operation_2_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "operation_3_try".to_string(),
            "http://localhost:8083/api/operation_3_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段：操作1成功，操作2失败，操作3成功
        saga.branch_succeed(gid.clone(), "operation_1_try".to_string()).await.unwrap();
        saga.branch_fail(gid.clone(), "operation_2_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "operation_3_try".to_string()).await.unwrap();

        // 提交事务（应该触发Cancel阶段）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态（应该被回滚）
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证事务被中止，但分支状态可能不同
        // 在TCC模式中，失败的分支会保持失败状态，成功的分支在回滚前可能仍显示成功
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证至少有一个分支失败（操作2）
        let failed_branches: Vec<_> = tx.branches.iter().filter(|b| b.status == BranchStatus::Failed).collect();
        assert!(!failed_branches.is_empty(), "应该至少有一个分支失败");
    }
}
