use crate::domain::{GlobalTxn, GlobalStatus, BranchTxn, BranchStatus};
use crate::repo::TxnRepo;
use anyhow::Result;
use std::sync::Arc;
use base64::Engine;
use prometheus::{Counter, Histogram};

pub struct SagaManager<R: TxnRepo> {
    repo: Arc<R>,
    exec: ExecConfig,
    metrics: Option<Arc<SagaMetrics>>,
}

#[derive(Clone, Copy)]
pub struct ExecConfig {
    pub request_timeout_secs: u64,
    pub retry_interval_secs: u64,
    pub timeout_to_fail_secs: u64,
    pub branch_parallelism: usize,
}

#[derive(Clone)]
pub struct SagaMetrics {
    pub branch_success_total: Counter,
    pub branch_failure_total: Counter,
    pub branch_latency_seconds: Histogram,
}

impl<R: TxnRepo> SagaManager<R> {
    pub fn new(repo: Arc<R>) -> Self { Self { repo, exec: ExecConfig { request_timeout_secs: 3, retry_interval_secs: 10, timeout_to_fail_secs: 35, branch_parallelism: 8 }, metrics: None } }

    pub fn with_exec_config(mut self, exec: ExecConfig) -> Self { self.exec = exec; self }
    pub fn with_metrics(mut self, metrics: Arc<SagaMetrics>) -> Self { self.metrics = Some(metrics); self }

    pub async fn start(&self, gid: String, mode: String, payload: Vec<u8>) -> Result<String> {
        let gid = if gid.is_empty() { uuid::Uuid::new_v4().to_string() } else { gid };
        let now = chrono::Utc::now().timestamp();
        let mode = if mode.is_empty() { "saga".to_string() } else { mode };
        let tx = GlobalTxn::new(gid.clone(), mode, payload, now);
        self.repo.save(&tx).await?;
        Ok(gid)
    }

    pub async fn submit(&self, gid: String) -> Result<()> {
        if let Some(mut tx) = self.repo.load(&gid).await? {
            // mark committing first
            tx.status = GlobalStatus::Committing;
            tx.updated_unix = chrono::Utc::now().timestamp();
            self.repo.save(&tx).await?;

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(self.exec.request_timeout_secs))
                .build()?;
            // execute prepared branches with concurrency limit
            let sem = Arc::new(tokio::sync::Semaphore::new(self.exec.branch_parallelism.max(1)));
            let mut handles = Vec::new();
            for i in 0..tx.branches.len() {
                if tx.branches[i].status != BranchStatus::Prepared { continue; }
                let permit = sem.clone().acquire_owned().await.unwrap();
                let client = client.clone();
                let url = tx.branches[i].action.clone();
                let gid = tx.gid.clone();
                let branch_id = tx.branches[i].branch_id.clone();
                let mode = tx.mode.clone();
                let payload = tx.payload.clone();
                let exec = self.exec;
                let metrics = self.metrics.clone();
                handles.push(tokio::spawn(async move {
                    let _permit = permit;
                    let body = serde_json::json!({ "gid": gid, "branch_id": branch_id, "mode": mode, "payload": base64::engine::general_purpose::STANDARD.encode(&payload) });
                    let start = std::time::Instant::now();
                    loop {
                        if let Ok(resp) = client.post(&url).json(&body).send().await {
                                if resp.status().is_success() {
                                    if let Some(m) = &metrics { m.branch_success_total.inc(); m.branch_latency_seconds.observe(start.elapsed().as_secs_f64()); }
                                    return true;
                                }
                        }
                        if start.elapsed() >= std::time::Duration::from_secs(exec.timeout_to_fail_secs) { return false; }
                        tokio::time::sleep(std::time::Duration::from_secs(exec.retry_interval_secs)).await;
                    }
                }));
            }
            let prepared_indices: Vec<usize> = tx.branches.iter().enumerate().filter(|(_, b)| b.status == BranchStatus::Prepared).map(|(i, _)| i).collect();
            for (i, h) in prepared_indices.into_iter().zip(handles.into_iter()) {
                let ok = h.await.unwrap_or(false);
                if ok {
                    tx.branches[i].status = BranchStatus::Succeed;
                } else {
                    if let Some(m) = &self.metrics { m.branch_failure_total.inc(); m.branch_latency_seconds.observe(0.0); }
                    tx.branches[i].status = BranchStatus::Failed;
                }
            }
            tx.updated_unix = chrono::Utc::now().timestamp();

            // persist branch updates after execution loop
            self.repo.save(&tx).await?;

            // finalize global status
            if tx.branches.iter().all(|b| b.status == BranchStatus::Succeed) {
                tx.status = GlobalStatus::Committed;
            } else if tx.branches.iter().any(|b| b.status == BranchStatus::Failed) {
                tx.status = GlobalStatus::Aborted;
            } else {
                tx.status = GlobalStatus::Committed; // no branches
            }
            tx.updated_unix = chrono::Utc::now().timestamp();
            self.repo.save(&tx).await?;
        }
        Ok(())
    }

    pub async fn abort(&self, gid: String) -> Result<()> {
        if let Some(mut tx) = self.repo.load(&gid).await? {
            // naive rollback: mark non-succeed branches as Failed
            for b in tx.branches.iter_mut() {
                if b.status != BranchStatus::Succeed {
                    b.status = BranchStatus::Failed;
                }
            }
            tx.status = GlobalStatus::Aborted;
            tx.updated_unix = chrono::Utc::now().timestamp();
            self.repo.save(&tx).await?;
        }
        Ok(())
    }

    pub async fn get(&self, gid: &str) -> Result<Option<GlobalTxn>> {
        self.repo.load(gid).await
    }
    pub async fn list(&self, limit: usize, offset: usize, status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>> {
        self.repo.list(limit, offset, status).await
    }

    pub async fn add_branch(&self, gid: String, branch_id: String, action: String) -> Result<()> {
        if let Some(mut tx) = self.repo.load(&gid).await? {
            if !tx.branches.iter().any(|b| b.branch_id == branch_id) {
                tx.branches.push(BranchTxn { branch_id, action, status: BranchStatus::Prepared });
                tx.updated_unix = chrono::Utc::now().timestamp();
                self.repo.save(&tx).await?;
            }
        }
        Ok(())
    }

    pub async fn branch_succeed(&self, gid: String, branch_id: String) -> Result<()> {
        if let Some(mut tx) = self.repo.load(&gid).await? {
            if let Some(b) = tx.branches.iter_mut().find(|b| b.branch_id == branch_id) {
                b.status = BranchStatus::Succeed;
                tx.updated_unix = chrono::Utc::now().timestamp();
                self.repo.save(&tx).await?;
            }
        }
        Ok(())
    }

    pub async fn branch_fail(&self, gid: String, branch_id: String) -> Result<()> {
        if let Some(mut tx) = self.repo.load(&gid).await? {
            if let Some(b) = tx.branches.iter_mut().find(|b| b.branch_id == branch_id) {
                b.status = BranchStatus::Failed;
                tx.updated_unix = chrono::Utc::now().timestamp();
                self.repo.save(&tx).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::TxnRepo;
    use crate::domain::GlobalTxn;
    use crate::storage::KV;
    use axum::{Router, routing::post, Json};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone, Default)]
    struct MemKv { inner: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>> }

    #[async_trait::async_trait]
    impl KV for MemKv {
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
                if k.starts_with(prefix) { out.push(v.clone()); }
                if out.len() >= limit { break; }
            }
            Ok(out)
        }
    }

    struct MemRepo { kv: MemKv }

    impl MemRepo { fn new() -> Self { Self { kv: MemKv::default() } } }

    #[async_trait::async_trait]
    impl TxnRepo for MemRepo {
        async fn save(&self, tx: &GlobalTxn) -> anyhow::Result<()> {
            let key = format!("gid:{}", tx.gid);
            let val = serde_json::to_vec(tx)?;
            self.kv.put(key.as_bytes(), &val).await
        }
        async fn load(&self, gid: &str) -> anyhow::Result<Option<GlobalTxn>> {
            let key = format!("gid:{}", gid);
            let Some(val) = self.kv.get(key.as_bytes()).await? else { return Ok(None) };
            Ok(Some(serde_json::from_slice(&val)?))
        }
        async fn list(&self, limit: usize, offset: usize, _status: Option<GlobalStatus>) -> anyhow::Result<Vec<GlobalTxn>> {
            let vals = self.kv.scan_prefix(b"gid:", offset + limit).await?;
            let mut out = Vec::new();
            for v in vals.into_iter().skip(offset) { out.push(serde_json::from_slice(&v)?); }
            Ok(out)
        }
    }

    async fn spawn_test_server() -> String {
        async fn ok(Json::<serde_json::Value>(_): Json<serde_json::Value>) -> &'static str { "ok" }
        async fn fail(Json::<serde_json::Value>(_): Json<serde_json::Value>) -> axum::http::StatusCode { axum::http::StatusCode::INTERNAL_SERVER_ERROR }
        let app = Router::new().route("/ok", post(ok)).route("/fail", post(fail));
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        format!("http://{}:{}", addr.ip(), addr.port())
    }

    #[tokio::test]
    async fn submit_executes_branches_and_commits_when_all_ok() {
        let base = spawn_test_server().await;
        let repo = Arc::new(MemRepo::new());
        let saga = SagaManager::new(repo.clone()).with_exec_config(ExecConfig { request_timeout_secs: 2, retry_interval_secs: 1, timeout_to_fail_secs: 3, branch_parallelism: 4 });
        let gid = saga.start("".into(), "saga".into(), b"test".to_vec()).await.unwrap();
        // add two ok branches
        saga.add_branch(gid.clone(), "b1".into(), format!("{}/ok", base)).await.unwrap();
        saga.add_branch(gid.clone(), "b2".into(), format!("{}/ok", base)).await.unwrap();
        saga.submit(gid.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Committed);
        assert!(tx.branches.iter().all(|b| b.status == BranchStatus::Succeed));
    }

    #[tokio::test]
    async fn submit_aborts_when_any_branch_fails() {
        let base = spawn_test_server().await;
        let repo = Arc::new(MemRepo::new());
        let saga = SagaManager::new(repo.clone()).with_exec_config(ExecConfig { request_timeout_secs: 1, retry_interval_secs: 1, timeout_to_fail_secs: 2, branch_parallelism: 4 });
        let gid = saga.start("".into(), "saga".into(), b"test".to_vec()).await.unwrap();
        saga.add_branch(gid.clone(), "b1".into(), format!("{}/ok", base)).await.unwrap();
        saga.add_branch(gid.clone(), "b2".into(), format!("{}/fail", base)).await.unwrap();
        saga.submit(gid.clone()).await.unwrap();
        let tx = saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(tx.status, GlobalStatus::Aborted);
        assert!(tx.branches.iter().any(|b| b.status == BranchStatus::Failed));
    }
}

