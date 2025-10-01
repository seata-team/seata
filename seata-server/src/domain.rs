use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlobalStatus {
    Submitted,
    Aborting,
    Aborted,
    Committing,
    Committed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BranchStatus {
    Prepared,
    Succeed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchTxn {
    pub branch_id: String,
    pub action: String,
    pub status: BranchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTxn {
    pub gid: String,
    pub mode: String,
    pub status: GlobalStatus,
    pub payload: Vec<u8>,
    pub branches: Vec<BranchTxn>,
    pub updated_unix: i64,
    pub created_unix: i64,
}

impl GlobalTxn {
    pub fn new(gid: String, mode: String, payload: Vec<u8>, now_unix: i64) -> Self {
        Self {
            gid,
            mode,
            status: GlobalStatus::Submitted,
            payload,
            branches: Vec::new(),
            updated_unix: now_unix,
            created_unix: now_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BarrierPhase {
    Try,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarrierKey {
    pub gid: String,
    pub branch_id: String,
    pub phase: BarrierPhase,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_global_status_serialization() {
        let statuses = vec![
            GlobalStatus::Submitted,
            GlobalStatus::Aborting,
            GlobalStatus::Aborted,
            GlobalStatus::Committing,
            GlobalStatus::Committed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: GlobalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_branch_status_serialization() {
        let statuses = vec![
            BranchStatus::Prepared,
            BranchStatus::Succeed,
            BranchStatus::Failed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: BranchStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_barrier_phase_serialization() {
        let phases = vec![
            BarrierPhase::Try,
            BarrierPhase::Confirm,
            BarrierPhase::Cancel,
        ];

        for phase in phases {
            let json = serde_json::to_string(&phase).unwrap();
            let deserialized: BarrierPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(phase, deserialized);
        }
    }

    #[test]
    fn test_global_txn_creation() {
        let now = chrono::Utc::now().timestamp();
        let gid = "test-gid".to_string();
        let mode = "saga".to_string();
        let payload = b"test payload".to_vec();

        let txn = GlobalTxn::new(gid.clone(), mode.clone(), payload.clone(), now);

        assert_eq!(txn.gid, gid);
        assert_eq!(txn.mode, mode);
        assert_eq!(txn.payload, payload);
        assert_eq!(txn.status, GlobalStatus::Submitted);
        assert_eq!(txn.branches.len(), 0);
        assert_eq!(txn.updated_unix, now);
        assert_eq!(txn.created_unix, now);
    }

    #[test]
    fn test_branch_txn_creation() {
        let branch = BranchTxn {
            branch_id: "branch-1".to_string(),
            action: "http://example.com/action".to_string(),
            status: BranchStatus::Prepared,
        };

        assert_eq!(branch.branch_id, "branch-1");
        assert_eq!(branch.action, "http://example.com/action");
        assert_eq!(branch.status, BranchStatus::Prepared);
    }

    #[test]
    fn test_barrier_key_creation() {
        let key = BarrierKey {
            gid: "test-gid".to_string(),
            branch_id: "branch-1".to_string(),
            phase: BarrierPhase::Try,
        };

        assert_eq!(key.gid, "test-gid");
        assert_eq!(key.branch_id, "branch-1");
        assert_eq!(key.phase, BarrierPhase::Try);
    }

    #[test]
    fn test_global_txn_serialization() {
        let now = chrono::Utc::now().timestamp();
        let txn = GlobalTxn {
            gid: "test-gid".to_string(),
            mode: "saga".to_string(),
            status: GlobalStatus::Submitted,
            payload: b"test payload".to_vec(),
            branches: vec![
                BranchTxn {
                    branch_id: "branch-1".to_string(),
                    action: "http://example.com/action1".to_string(),
                    status: BranchStatus::Prepared,
                },
                BranchTxn {
                    branch_id: "branch-2".to_string(),
                    action: "http://example.com/action2".to_string(),
                    status: BranchStatus::Succeed,
                },
            ],
            updated_unix: now,
            created_unix: now,
        };

        let json = serde_json::to_string(&txn).unwrap();
        let deserialized: GlobalTxn = serde_json::from_str(&json).unwrap();
        
        assert_eq!(txn.gid, deserialized.gid);
        assert_eq!(txn.mode, deserialized.mode);
        assert_eq!(txn.status, deserialized.status);
        assert_eq!(txn.payload, deserialized.payload);
        assert_eq!(txn.branches.len(), deserialized.branches.len());
        assert_eq!(txn.updated_unix, deserialized.updated_unix);
        assert_eq!(txn.created_unix, deserialized.created_unix);
    }

    #[test]
    fn test_status_equality() {
        assert_eq!(GlobalStatus::Submitted, GlobalStatus::Submitted);
        assert_ne!(GlobalStatus::Submitted, GlobalStatus::Committed);
        
        assert_eq!(BranchStatus::Prepared, BranchStatus::Prepared);
        assert_ne!(BranchStatus::Prepared, BranchStatus::Succeed);
        
        assert_eq!(BarrierPhase::Try, BarrierPhase::Try);
        assert_ne!(BarrierPhase::Try, BarrierPhase::Confirm);
    }

    #[test]
    fn test_status_string_representation() {
        // Test that serialized statuses use SCREAMING_SNAKE_CASE
        let submitted_json = serde_json::to_string(&GlobalStatus::Submitted).unwrap();
        assert!(submitted_json.contains("SUBMITTED"));
        
        let prepared_json = serde_json::to_string(&BranchStatus::Prepared).unwrap();
        assert!(prepared_json.contains("PREPARED"));
    }
}