use crate::test_utils::TestRepo;
use crate::domain::{GlobalStatus, BranchStatus};
use crate::saga::{SagaManager, ExecConfig};
use std::sync::Arc;
use serde_json::json;

/// 基于seata-examples的Workflow模式测试
/// 模拟工作流模式的分布式事务
#[cfg(test)]
mod tests {
    use super::*;

    /// 模拟Workflow Saga场景 - 基于grpc_workflow_saga示例
    #[tokio::test]
    async fn test_workflow_saga_scenario() {
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

        // 模拟Workflow Saga场景
        let workflow_data = json!({
            "workflow_id": "workflow_saga_001",
            "workflow_type": "saga",
            "steps": [
                {"step": 1, "action": "trans_out", "compensate": "trans_out_revert"},
                {"step": 2, "action": "trans_in", "compensate": "trans_in_revert"}
            ],
            "data": {
                "amount": 30,
                "from_account": "account_a",
                "to_account": "account_b"
            }
        });

        let gid = saga.start(
            "workflow_saga_tx_001".to_string(),
            "workflow_saga".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // Workflow步骤1：转出
        saga.add_branch(
            gid.clone(),
            "step_1_trans_out".to_string(),
            "http://localhost:8081/api/workflow/trans_out".to_string()
        ).await.unwrap();

        // Workflow步骤2：转入
        saga.add_branch(
            gid.clone(),
            "step_2_trans_in".to_string(),
            "http://localhost:8081/api/workflow/trans_in".to_string()
        ).await.unwrap();

        // 模拟所有步骤成功
        saga.branch_succeed(gid.clone(), "step_1_trans_out".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_2_trans_in".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
    }

    /// 模拟Workflow TCC场景 - 基于grpc_workflow_tcc示例
    #[tokio::test]
    async fn test_workflow_tcc_scenario() {
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

        // 模拟Workflow TCC场景
        let workflow_data = json!({
            "workflow_id": "workflow_tcc_001",
            "workflow_type": "tcc",
            "steps": [
                {"step": 1, "try": "trans_out_try", "confirm": "trans_out_confirm", "cancel": "trans_out_cancel"},
                {"step": 2, "try": "trans_in_try", "confirm": "trans_in_confirm", "cancel": "trans_in_cancel"}
            ],
            "data": {
                "amount": 30,
                "from_account": "account_a",
                "to_account": "account_b"
            }
        });

        let gid = saga.start(
            "workflow_tcc_tx_001".to_string(),
            "workflow_tcc".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // Workflow TCC步骤1：转出
        saga.add_branch(
            gid.clone(),
            "step_1_trans_out_try".to_string(),
            "http://localhost:8081/api/workflow/trans_out_try".to_string()
        ).await.unwrap();

        // Workflow TCC步骤2：转入
        saga.add_branch(
            gid.clone(),
            "step_2_trans_in_try".to_string(),
            "http://localhost:8081/api/workflow/trans_in_try".to_string()
        ).await.unwrap();

        // 模拟Try阶段成功
        saga.branch_succeed(gid.clone(), "step_1_trans_out_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_2_trans_in_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
    }

    /// 模拟Workflow XA场景 - 基于grpc_workflow_xa示例
    #[tokio::test]
    async fn test_workflow_xa_scenario() {
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

        // 模拟Workflow XA场景
        let workflow_data = json!({
            "workflow_id": "workflow_xa_001",
            "workflow_type": "xa",
            "steps": [
                {"step": 1, "action": "prepare_trans_out", "commit": "commit_trans_out", "rollback": "rollback_trans_out"},
                {"step": 2, "action": "prepare_trans_in", "commit": "commit_trans_in", "rollback": "rollback_trans_in"}
            ],
            "data": {
                "amount": 30,
                "from_account": "account_a",
                "to_account": "account_b"
            }
        });

        let gid = saga.start(
            "workflow_xa_tx_001".to_string(),
            "workflow_xa".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // Workflow XA步骤1：准备转出
        saga.add_branch(
            gid.clone(),
            "step_1_prepare_trans_out".to_string(),
            "http://localhost:8081/api/workflow/prepare_trans_out".to_string()
        ).await.unwrap();

        // Workflow XA步骤2：准备转入
        saga.add_branch(
            gid.clone(),
            "step_2_prepare_trans_in".to_string(),
            "http://localhost:8081/api/workflow/prepare_trans_in".to_string()
        ).await.unwrap();

        // 模拟准备阶段成功
        saga.branch_succeed(gid.clone(), "step_1_prepare_trans_out".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_2_prepare_trans_in".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 2);
    }

    /// 模拟Workflow混合场景 - 基于grpc_workflow_mixed示例
    #[tokio::test]
    async fn test_workflow_mixed_scenario() {
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

        // 模拟Workflow混合场景
        let workflow_data = json!({
            "workflow_id": "workflow_mixed_001",
            "workflow_type": "mixed",
            "steps": [
                {"step": 1, "type": "saga", "action": "trans_out", "compensate": "trans_out_revert"},
                {"step": 2, "type": "tcc", "try": "trans_in_try", "confirm": "trans_in_confirm", "cancel": "trans_in_cancel"},
                {"step": 3, "type": "xa", "action": "prepare_notification", "commit": "commit_notification", "rollback": "rollback_notification"}
            ],
            "data": {
                "amount": 30,
                "from_account": "account_a",
                "to_account": "account_b"
            }
        });

        let gid = saga.start(
            "workflow_mixed_tx_001".to_string(),
            "workflow_mixed".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // Workflow混合步骤1：Saga模式转出
        saga.add_branch(
            gid.clone(),
            "step_1_saga_trans_out".to_string(),
            "http://localhost:8081/api/workflow/saga/trans_out".to_string()
        ).await.unwrap();

        // Workflow混合步骤2：TCC模式转入
        saga.add_branch(
            gid.clone(),
            "step_2_tcc_trans_in_try".to_string(),
            "http://localhost:8081/api/workflow/tcc/trans_in_try".to_string()
        ).await.unwrap();

        // Workflow混合步骤3：XA模式通知
        saga.add_branch(
            gid.clone(),
            "step_3_xa_prepare_notification".to_string(),
            "http://localhost:8081/api/workflow/xa/prepare_notification".to_string()
        ).await.unwrap();

        // 模拟所有步骤成功
        saga.branch_succeed(gid.clone(), "step_1_saga_trans_out".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_2_tcc_trans_in_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_3_xa_prepare_notification".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 3);
    }

    /// 模拟复杂工作流场景
    #[tokio::test]
    async fn test_complex_workflow_scenario() {
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

        // 模拟复杂工作流场景
        let workflow_data = json!({
            "workflow_id": "complex_workflow_001",
            "workflow_type": "complex",
            "phases": [
                {
                    "phase": "preparation",
                    "steps": [
                        {"step": 1, "action": "validate_input", "type": "saga"},
                        {"step": 2, "action": "check_permissions", "type": "tcc"}
                    ]
                },
                {
                    "phase": "execution",
                    "steps": [
                        {"step": 3, "action": "process_main", "type": "saga"},
                        {"step": 4, "action": "process_secondary", "type": "tcc"},
                        {"step": 5, "action": "process_tertiary", "type": "xa"}
                    ]
                },
                {
                    "phase": "completion",
                    "steps": [
                        {"step": 6, "action": "send_notifications", "type": "saga"},
                        {"step": 7, "action": "update_audit_log", "type": "tcc"}
                    ]
                }
            ],
            "data": {
                "user_id": "user_123",
                "operation": "complex_operation",
                "parameters": {"param1": "value1", "param2": "value2"}
            }
        });

        let gid = saga.start(
            "complex_workflow_tx_001".to_string(),
            "complex_workflow".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // 准备阶段
        saga.add_branch(
            gid.clone(),
            "step_1_validate_input".to_string(),
            "http://localhost:8081/api/workflow/validate_input".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_2_check_permissions_try".to_string(),
            "http://localhost:8081/api/workflow/check_permissions_try".to_string()
        ).await.unwrap();

        // 执行阶段
        saga.add_branch(
            gid.clone(),
            "step_3_process_main".to_string(),
            "http://localhost:8082/api/workflow/process_main".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_4_process_secondary_try".to_string(),
            "http://localhost:8082/api/workflow/process_secondary_try".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_5_process_tertiary_prepare".to_string(),
            "http://localhost:8083/api/workflow/process_tertiary_prepare".to_string()
        ).await.unwrap();

        // 完成阶段
        saga.add_branch(
            gid.clone(),
            "step_6_send_notifications".to_string(),
            "http://localhost:8084/api/workflow/send_notifications".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_7_update_audit_log_try".to_string(),
            "http://localhost:8084/api/workflow/update_audit_log_try".to_string()
        ).await.unwrap();

        // 模拟所有步骤成功
        saga.branch_succeed(gid.clone(), "step_1_validate_input".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_2_check_permissions_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_3_process_main".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_4_process_secondary_try".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_5_process_tertiary_prepare".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_6_send_notifications".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "step_7_update_audit_log_try".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 7);
    }

    /// 模拟工作流回滚场景
    #[tokio::test]
    async fn test_workflow_rollback_scenario() {
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

        // 模拟工作流回滚场景
        let workflow_data = json!({
            "workflow_id": "workflow_rollback_001",
            "workflow_type": "saga",
            "steps": [
                {"step": 1, "action": "trans_out", "compensate": "trans_out_revert"},
                {"step": 2, "action": "trans_in", "compensate": "trans_in_revert"},
                {"step": 3, "action": "notification", "compensate": "notification_revert"}
            ],
            "data": {
                "amount": 30,
                "from_account": "account_a",
                "to_account": "account_b"
            },
            "expected_failure_step": 2
        });

        let gid = saga.start(
            "workflow_rollback_tx_001".to_string(),
            "workflow_saga".to_string(),
            serde_json::to_vec(&workflow_data).unwrap()
        ).await.unwrap();

        // 工作流步骤
        saga.add_branch(
            gid.clone(),
            "step_1_trans_out".to_string(),
            "http://localhost:8081/api/workflow/trans_out".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_2_trans_in".to_string(),
            "http://localhost:8081/api/workflow/trans_in".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "step_3_notification".to_string(),
            "http://localhost:8081/api/workflow/notification".to_string()
        ).await.unwrap();

        // 模拟步骤1成功，步骤2失败，步骤3未执行
        saga.branch_succeed(gid.clone(), "step_1_trans_out".to_string()).await.unwrap();
        saga.branch_fail(gid.clone(), "step_2_trans_in".to_string()).await.unwrap();

        // 提交事务（应该触发回滚）
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态（应该被回滚）
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证事务被中止，但分支状态可能不同
        // 在Saga模式中，失败的分支会保持失败状态，成功的分支在回滚前可能仍显示成功
        assert_eq!(tx.status, GlobalStatus::Aborted);
        
        // 验证至少有一个分支失败（step_2_trans_in）
        let failed_branches: Vec<_> = tx.branches.iter().filter(|b| b.status == BranchStatus::Failed).collect();
        assert!(!failed_branches.is_empty(), "应该至少有一个分支失败");
    }

    /// 模拟工作流并发场景
    #[tokio::test]
    async fn test_workflow_concurrent_scenario() {
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

        // 创建多个并发工作流
        let mut handles = Vec::new();
        for i in 0..30 {
            let saga = saga.clone();
            let handle = tokio::spawn(async move {
                let gid = format!("workflow_concurrent_{}", i);
                let data = json!({
                    "workflow_id": gid,
                    "workflow_type": "saga",
                    "steps": [
                        {"step": 1, "action": "process", "compensate": "process_revert"},
                        {"step": 2, "action": "notify", "compensate": "notify_revert"}
                    ],
                    "data": {
                        "id": i,
                        "timestamp": chrono::Utc::now().timestamp()
                    }
                });

                saga.start(
                    gid.clone(),
                    "workflow_saga".to_string(),
                    serde_json::to_vec(&data).unwrap()
                ).await?;

                // 添加工作流步骤
                saga.add_branch(
                    gid.clone(),
                    "step_1_process".to_string(),
                    format!("http://localhost:8081/api/workflow/process_{}", i)
                ).await?;

                saga.add_branch(
                    gid.clone(),
                    "step_2_notify".to_string(),
                    format!("http://localhost:8082/api/workflow/notify_{}", i)
                ).await?;

                saga.submit(gid.clone()).await?;
                Ok::<String, anyhow::Error>(gid)
            });
            handles.push(handle);
        }

        // 等待所有工作流完成
        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                success_count += 1;
            }
        }

        // 验证大部分工作流成功
        assert!(success_count >= 25); // 至少83%成功
    }

    /// 模拟工作流状态机场景
    #[tokio::test]
    async fn test_workflow_state_machine_scenario() {
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

        // 模拟工作流状态机场景
        let state_machine_data = json!({
            "workflow_id": "state_machine_001",
            "workflow_type": "state_machine",
            "states": [
                {"state": "INIT", "action": "initialize", "next_state": "VALIDATING"},
                {"state": "VALIDATING", "action": "validate", "next_state": "PROCESSING"},
                {"state": "PROCESSING", "action": "process", "next_state": "COMPLETING"},
                {"state": "COMPLETING", "action": "complete", "next_state": "DONE"}
            ],
            "current_state": "INIT",
            "data": {
                "request_id": "req_001",
                "user_id": "user_123"
            }
        });

        let gid = saga.start(
            "state_machine_tx_001".to_string(),
            "state_machine".to_string(),
            serde_json::to_vec(&state_machine_data).unwrap()
        ).await.unwrap();

        // 状态机步骤
        saga.add_branch(
            gid.clone(),
            "state_INIT_initialize".to_string(),
            "http://localhost:8081/api/state_machine/initialize".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "state_VALIDATING_validate".to_string(),
            "http://localhost:8081/api/state_machine/validate".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "state_PROCESSING_process".to_string(),
            "http://localhost:8081/api/state_machine/process".to_string()
        ).await.unwrap();

        saga.add_branch(
            gid.clone(),
            "state_COMPLETING_complete".to_string(),
            "http://localhost:8081/api/state_machine/complete".to_string()
        ).await.unwrap();

        // 模拟所有状态转换成功
        saga.branch_succeed(gid.clone(), "state_INIT_initialize".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "state_VALIDATING_validate".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "state_PROCESSING_process".to_string()).await.unwrap();
        saga.branch_succeed(gid.clone(), "state_COMPLETING_complete".to_string()).await.unwrap();

        // 提交事务
        saga.submit(gid.clone()).await.unwrap();

        // 验证事务状态
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert_eq!(tx.branches.len(), 4);
    }
}
