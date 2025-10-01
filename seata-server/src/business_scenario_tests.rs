use crate::test_utils::TestRepo;
use crate::domain::{GlobalStatus, BranchStatus};
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;
use serde_json::json;

/// 基于seata-examples的业务场景测试
/// 模拟真实的分布式事务业务场景
#[cfg(test)]
mod tests {
    use super::*;

    /// 模拟转账业务场景 - 基于quick_start示例
    #[tokio::test]
    async fn test_transfer_scenario() {
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

        // 模拟转账场景：从账户A转30元到账户B
        let transfer_data = json!({
            "amount": 30,
            "from_account": "account_a",
            "to_account": "account_b"
        });

        let gid = saga.start(
            "transfer_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&transfer_data).unwrap()
        ).await.unwrap();

        // 添加转出操作
        saga.add_branch(
            gid.clone(),
            "trans_out".to_string(),
            "http://localhost:8081/api/trans_out".to_string()
        ).await.unwrap();

        // 添加转入操作
        saga.add_branch(
            gid.clone(),
            "trans_in".to_string(),
            "http://localhost:8081/api/trans_in".to_string()
        ).await.unwrap();

        // 模拟分支操作成功
        saga.branch_succeed(gid.clone(), "trans_out".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_in".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
        assert_eq!(tx.branches[0].branch_id, "trans_out");
        assert_eq!(tx.branches[1].branch_id, "trans_in");
    }

    /// 模拟转账回滚场景 - 基于http_saga_rollback示例
    #[tokio::test]
    async fn test_transfer_rollback_scenario() {
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

        // 模拟转账失败场景
        let transfer_data = json!({
            "amount": 30,
            "from_account": "account_a",
            "to_account": "account_b",
            "trans_in_result": "FAILURE" // 模拟转入失败
        });

        let gid = saga.start(
            "transfer_rollback_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&transfer_data).unwrap()
        ).await.unwrap();

        // 添加转出操作
        saga.add_branch(
            gid.clone(),
            "trans_out".to_string(),
            "http://localhost:8081/api/trans_out".to_string()
        ).await.unwrap();

        // 添加转入操作（会失败）
        saga.add_branch(
            gid.clone(),
            "trans_in".to_string(),
            "http://localhost:8081/api/trans_in".to_string()
        ).await.unwrap();

        // 手动设置转入失败
        saga.branch_fail(gid.clone(), "trans_in".to_string()).await.unwrap();

        // 提交事务（应该触发回滚）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态（应该被回滚）
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        let trans_out_branch = tx.branches.iter().find(|b| b.branch_id == "trans_out").unwrap();
        let trans_in_branch = tx.branches.iter().find(|b| b.branch_id == "trans_in").unwrap();
        
        assert_eq!(trans_out_branch.status, BranchStatus::Failed); // 应该被回滚
        assert_eq!(trans_in_branch.status, BranchStatus::Failed);
    }

    /// 模拟并发转账场景 - 基于http_concurrent_saga示例
    #[tokio::test]
    async fn test_concurrent_transfer_scenario() {
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

        let transfer_data = json!({
            "amount": 30,
            "from_account": "account_a",
            "to_account": "account_b"
        });

        let gid = saga.start(
            "concurrent_transfer_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&transfer_data).unwrap()
        ).await.unwrap();

        // 添加多个并发操作
        saga.add_branch(
            gid.clone(),
            "trans_out_1".to_string(),
            "http://localhost:8081/api/trans_out".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "trans_out_2".to_string(),
            "http://localhost:8081/api/trans_out".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "trans_in_1".to_string(),
            "http://localhost:8081/api/trans_in".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "trans_in_2".to_string(),
            "http://localhost:8081/api/trans_in".to_string()
        ).await.unwrap();

        // 模拟所有分支操作成功
        saga.branch_succeed(gid.clone(), "trans_out_1".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_out_2".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_in_1".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "trans_in_2".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }

    /// 模拟订单系统场景 - 基于真实业务场景
    #[tokio::test]
    async fn test_order_system_scenario() {
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

        // 模拟订单创建场景
        let order_data = json!({
            "order_id": "order_001",
            "user_id": "user_123",
            "product_id": "product_456",
            "quantity": 2,
            "amount": 100.0
        });

        let gid = saga.start(
            "order_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&order_data).unwrap()
        ).await.unwrap();

        // 1. 创建订单
        saga.add_branch(
            gid.clone(),
            "create_order".to_string(),
            "http://localhost:8081/api/orders/create".to_string()
        ).await.unwrap();

        // 2. 扣减库存
        saga.add_branch(
            gid.clone(),
            "reduce_inventory".to_string(),
            "http://localhost:8082/api/inventory/reduce".to_string()
        ).await.unwrap();

        // 3. 扣减用户余额
        saga.add_branch(
            gid.clone(),
            "deduct_balance".to_string(),
            "http://localhost:8083/api/balance/deduct".to_string()
        ).await.unwrap();

        // 4. 发送通知
        saga.add_branch(
            gid.clone(),
            "send_notification".to_string(),
            "http://localhost:8084/api/notifications/send".to_string()
        ).await.unwrap();

        // 模拟所有分支操作成功
        saga.branch_succeed(gid.clone(), "create_order".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "reduce_inventory".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "deduct_balance".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "send_notification".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }

    /// 模拟订单系统回滚场景
    #[tokio::test]
    async fn test_order_system_rollback_scenario() {
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
            "order_id": "order_002",
            "user_id": "user_123",
            "product_id": "product_456",
            "quantity": 2,
            "amount": 100.0
        });

        let gid = saga.start(
            "order_rollback_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&order_data).unwrap()
        ).await.unwrap();

        // 1. 创建订单
        saga.add_branch(
            gid.clone(),
            "create_order".to_string(),
            "http://localhost:8081/api/orders/create".to_string()
        ).await.unwrap();

        // 2. 扣减库存
        saga.add_branch(
            gid.clone(),
            "reduce_inventory".to_string(),
            "http://localhost:8082/api/inventory/reduce".to_string()
        ).await.unwrap();

        // 3. 扣减用户余额（模拟失败）
        saga.add_branch(
            gid.clone(),
            "deduct_balance".to_string(),
            "http://localhost:8083/api/balance/deduct".to_string()
        ).await.unwrap();

        // 手动设置余额扣减失败
        saga.branch_fail(gid.clone(), "deduct_balance".to_string()).await.unwrap();

        // 提交事务（应该触发回滚）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态（应该被回滚）
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证所有分支都被回滚
        for branch in &tx.branches {
            assert_eq!(branch.status, BranchStatus::Failed);
        }
    }

    /// 模拟支付系统场景
    #[tokio::test]
    async fn test_payment_system_scenario() {
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
            "payment_id": "payment_001",
            "user_id": "user_123",
            "amount": 50.0,
            "currency": "USD",
            "payment_method": "credit_card"
        });

        let gid = saga.start(
            "payment_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&payment_data).unwrap()
        ).await.unwrap();

        // 1. 验证支付方式
        saga.add_branch(
            gid.clone(),
            "validate_payment_method".to_string(),
            "http://localhost:8081/api/payments/validate".to_string()
        ).await.unwrap();

        // 2. 处理支付
        saga.add_branch(
            gid.clone(),
            "process_payment".to_string(),
            "http://localhost:8082/api/payments/process".to_string()
        ).await.unwrap();

        // 3. 更新账户余额
        saga.add_branch(
            gid.clone(),
            "update_balance".to_string(),
            "http://localhost:8083/api/balance/update".to_string()
        ).await.unwrap();

        // 4. 记录交易日志
        saga.add_branch(
            gid.clone(),
            "log_transaction".to_string(),
            "http://localhost:8084/api/logs/transaction".to_string()
        ).await.unwrap();

        // 模拟所有分支操作成功
        saga.branch_succeed(gid.clone(), "validate_payment_method".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "process_payment".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_balance".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "log_transaction".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }

    /// 模拟库存管理系统场景
    #[tokio::test]
    async fn test_inventory_management_scenario() {
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
            "operation": "stock_in"
        });

        let gid = saga.start(
            "inventory_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&inventory_data).unwrap()
        ).await.unwrap();

        // 1. 验证产品信息
        saga.add_branch(
            gid.clone(),
            "validate_product".to_string(),
            "http://localhost:8081/api/products/validate".to_string()
        ).await.unwrap();

        // 2. 更新库存
        saga.add_branch(
            gid.clone(),
            "update_inventory".to_string(),
            "http://localhost:8082/api/inventory/update".to_string()
        ).await.unwrap();

        // 3. 更新仓库记录
        saga.add_branch(
            gid.clone(),
            "update_warehouse".to_string(),
            "http://localhost:8083/api/warehouse/update".to_string()
        ).await.unwrap();

        // 4. 发送库存变更通知
        saga.add_branch(
            gid.clone(),
            "notify_inventory_change".to_string(),
            "http://localhost:8084/api/notifications/inventory".to_string()
        ).await.unwrap();

        // 模拟所有分支操作成功
        saga.branch_succeed(gid.clone(), "validate_product".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_inventory".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_warehouse".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "notify_inventory_change".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }

    /// 模拟多服务协调场景
    #[tokio::test]
    async fn test_multi_service_coordination_scenario() {
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

        let coordination_data = json!({
            "operation_id": "op_001",
            "services": ["user_service", "order_service", "payment_service", "notification_service"],
            "data": {
                "user_id": "user_123",
                "order_id": "order_456",
                "amount": 100.0
            }
        });

        let gid = saga.start(
            "coordination_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&coordination_data).unwrap()
        ).await.unwrap();

        // 协调多个服务
        let services = vec![
            ("user_service", "http://localhost:8081/api/users/update"),
            ("order_service", "http://localhost:8082/api/orders/process"),
            ("payment_service", "http://localhost:8083/api/payments/charge"),
            ("notification_service", "http://localhost:8084/api/notifications/send"),
        ];

        for (service_name, endpoint) in services {
            saga.add_branch(
                gid.clone(),
                service_name.to_string(),
                endpoint.to_string()
            ).await.unwrap();
        }

        // 模拟所有服务操作成功
        saga.branch_succeed(gid.clone(), "user_service".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "order_service".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "payment_service".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "notification_service".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }

    /// 模拟高并发场景测试
    #[tokio::test]
    async fn test_high_concurrency_scenario() {
        let repo = Arc::new(TestRepo::new());
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 32, // 高并发
                })
        );

        // 创建多个并发事务
        let mut handles = Vec::new();
        for i in 0..50 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("concurrent_tx_{}", i);
                let data = json!({
                    "transaction_id": gid,
                    "amount": 10 + i,
                    "timestamp": chrono::Utc::now().timestamp()
                });

                saga.start(
                    gid.clone(),
                    "saga".to_string(),
                    serde_json::to_vec(&data).unwrap()
                ).await?;

                // 添加多个分支
                for j in 0..3 {
                    saga.add_branch(
                        gid.clone(),
                        format!("branch_{}", j),
                        format!("http://localhost:808{}/api/process", j + 1)
                    ).await?;
                }

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
        assert!(success_count >= 40); // 至少80%成功
    }

    /// 模拟数据一致性场景
    #[tokio::test]
    async fn test_data_consistency_scenario() {
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
            "data_id": "data_001",
            "operations": [
                {"type": "create", "table": "users", "data": {"id": 1, "name": "Alice"}},
                {"type": "update", "table": "profiles", "data": {"user_id": 1, "email": "alice@example.com"}},
                {"type": "insert", "table": "logs", "data": {"user_id": 1, "action": "created"}}
            ]
        });

        let gid = saga.start(
            "consistency_tx_001".to_string(),
            "saga".to_string(),
            serde_json::to_vec(&consistency_data).unwrap()
        ).await.unwrap();

        // 确保数据一致性的多个操作
        saga.add_branch(
            gid.clone(),
            "create_user".to_string(),
            "http://localhost:8081/api/users/create".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "update_profile".to_string(),
            "http://localhost:8082/api/profiles/update".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "log_operation".to_string(),
            "http://localhost:8083/api/logs/insert".to_string()
        ).await.unwrap();

        // 模拟所有操作成功
        saga.branch_succeed(gid.clone(), "create_user".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "update_profile".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "log_operation".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }
}
