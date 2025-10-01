use crate::domain::{GlobalTxn, GlobalStatus};
use crate::repo::TxnRepo;
use crate::storage::KV;
use crate::saga::{SagaManager, ExecConfig};
use crate::barrier::{Barrier, BarrierOps};
use axum::{Router, routing::post, Json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use async_trait::async_trait;

/// In-memory KV store for testing
#[derive(Clone, Default)]
pub struct TestKV {
    inner: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

#[async_trait]
impl KV for TestKV {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.inner.lock().await.get(key).cloned())
    }
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.inner.lock().await.insert(key.to_vec(), value.to_vec());
        Ok(())
    }
    async fn delete(&self, key: &[u8]) -> Result<()> {
        self.inner.lock().await.remove(key);
        Ok(())
    }
    async fn scan_prefix(&self, prefix: &[u8], limit: usize) -> Result<Vec<Vec<u8>>> {
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

/// In-memory repository for testing
pub struct TestRepo {
    kv: TestKV,
}

impl TestRepo {
    pub fn new() -> Self {
        Self { kv: TestKV::default() }
    }
}

#[async_trait]
impl TxnRepo for TestRepo {
    async fn save(&self, tx: &GlobalTxn) -> Result<()> {
        let key = format!("gid:{}", tx.gid);
        let val = serde_json::to_vec(tx)?;
        self.kv.put(key.as_bytes(), &val).await
    }
    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>> {
        let key = format!("gid:{}", gid);
        let Some(val) = self.kv.get(key.as_bytes()).await? else { return Ok(None) };
        Ok(Some(serde_json::from_slice(&val)?))
    }
    async fn list(&self, limit: usize, offset: usize, _status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>> {
        let vals = self.kv.scan_prefix(b"gid:", offset + limit).await?;
        let mut out = Vec::new();
        for v in vals.into_iter().skip(offset) {
            out.push(serde_json::from_slice(&v)?);
        }
        Ok(out)
    }
}

/// Test server utilities
pub struct TestServer {
    pub base_url: String,
    pub saga: Arc<SagaManager<TestRepo>>,
    pub barrier: Arc<dyn BarrierOps>,
}

impl TestServer {
    pub async fn new() -> Self {
        let repo = Arc::new(TestRepo::new());
        let kv = TestKV::default();
        let barrier = Arc::new(Barrier::new(kv)) as Arc<dyn BarrierOps>;
        
        let saga = Arc::new(
            SagaManager::new(repo)
                .with_exec_config(ExecConfig {
                    request_timeout_secs: 1,
                    retry_interval_secs: 1,
                    timeout_to_fail_secs: 3,
                    branch_parallelism: 4,
                })
        );

        let base_url = spawn_test_server().await;
        
        Self {
            base_url,
            saga,
            barrier,
        }
    }
}

/// Spawn a test HTTP server that responds to branch actions
async fn spawn_test_server() -> String {
    async fn ok_handler(Json::<serde_json::Value>(_): Json<serde_json::Value>) -> &'static str { 
        "ok" 
    }
    
    async fn fail_handler(Json::<serde_json::Value>(_): Json<serde_json::Value>) -> axum::http::StatusCode { 
        axum::http::StatusCode::INTERNAL_SERVER_ERROR 
    }
    
    async fn timeout_handler(Json::<serde_json::Value>(_): Json<serde_json::Value>) -> axum::http::StatusCode {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        axum::http::StatusCode::OK
    }

    let app = Router::new()
        .route("/ok", post(ok_handler))
        .route("/fail", post(fail_handler))
        .route("/timeout", post(timeout_handler));
    
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { 
        axum::serve(listener, app).await.unwrap(); 
    });
    
    format!("http://{}:{}", addr.ip(), addr.port())
}

/// HTTP client for testing API endpoints
pub struct TestClient {
    client: reqwest::Client,
    base_url: String,
}

impl TestClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn start(&self, gid: Option<String>, mode: Option<String>, payload: Option<Vec<u8>>) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "gid": gid,
            "mode": mode,
            "payload": payload
        });
        
        let response = self.client
            .post(&format!("{}/api/start", self.base_url))
            .json(&body)
            .send()
            .await?;
        
        Ok(response.json().await?)
    }

    pub async fn submit(&self, gid: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({ "gid": gid });
        
        Ok(self.client
            .post(&format!("{}/api/submit", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn abort(&self, gid: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({ "gid": gid });
        
        Ok(self.client
            .post(&format!("{}/api/abort", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn add_branch(&self, gid: String, branch_id: String, action: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({
            "gid": gid,
            "branch_id": branch_id,
            "action": action
        });
        
        Ok(self.client
            .post(&format!("{}/api/branch/add", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn branch_try(&self, gid: String, branch_id: String, action: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({
            "gid": gid,
            "branch_id": branch_id,
            "action": action
        });
        
        Ok(self.client
            .post(&format!("{}/api/branch/try", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn branch_succeed(&self, gid: String, branch_id: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({
            "gid": gid,
            "branch_id": branch_id
        });
        
        Ok(self.client
            .post(&format!("{}/api/branch/succeed", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn branch_fail(&self, gid: String, branch_id: String) -> Result<reqwest::Response> {
        let body = serde_json::json!({
            "gid": gid,
            "branch_id": branch_id
        });
        
        Ok(self.client
            .post(&format!("{}/api/branch/fail", self.base_url))
            .json(&body)
            .send()
            .await?)
    }

    pub async fn get_tx(&self, gid: String) -> Result<reqwest::Response> {
        Ok(self.client
            .get(&format!("{}/api/tx/{}", self.base_url, gid))
            .send()
            .await?)
    }

    pub async fn list_tx(&self, limit: Option<usize>, offset: Option<usize>, status: Option<String>) -> Result<reqwest::Response> {
        let mut url = format!("{}/api/tx", self.base_url);
        let mut params = Vec::new();
        
        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }
        if let Some(offset) = offset {
            params.push(format!("offset={}", offset));
        }
        if let Some(status) = status {
            params.push(format!("status={}", status));
        }
        
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        
        Ok(self.client.get(&url).send().await?)
    }

    pub async fn health(&self) -> Result<String> {
        let response = self.client
            .get(&format!("{}/health", self.base_url))
            .send()
            .await?;
        
        Ok(response.text().await?)
    }

    pub async fn metrics(&self) -> Result<String> {
        let response = self.client
            .get(&format!("{}/metrics", self.base_url))
            .send()
            .await?;
        
        Ok(response.text().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_kv() {
        let kv = TestKV::default();
        
        // Test basic operations
        assert!(kv.get(b"key1").await.unwrap().is_none());
        
        kv.put(b"key1", b"value1").await.unwrap();
        assert_eq!(kv.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));
        
        kv.delete(b"key1").await.unwrap();
        assert!(kv.get(b"key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_test_repo() {
        let repo = TestRepo::new();
        let now = chrono::Utc::now().timestamp();
        
        let txn = GlobalTxn::new("test-gid".to_string(), "saga".to_string(), b"payload".to_vec(), now);
        
        // Save and load
        repo.save(&txn).await.unwrap();
        let loaded = repo.load("test-gid").await.unwrap().unwrap();
        
        assert_eq!(txn.gid, loaded.gid);
        assert_eq!(txn.mode, loaded.mode);
        assert_eq!(txn.payload, loaded.payload);
    }

    #[tokio::test]
    async fn test_test_server() {
        let server = TestServer::new().await;
        
        // Test that we can create a transaction
        let gid = server.saga.start("test-gid".to_string(), "saga".to_string(), b"payload".to_vec()).await.unwrap();
        assert_eq!(gid, "test-gid");
        
        // Test that we can retrieve it
        let txn = server.saga.get(&gid).await.unwrap().unwrap();
        assert_eq!(txn.gid, "test-gid");
    }
}
