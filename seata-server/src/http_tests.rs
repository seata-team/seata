use crate::test_utils::{TestServer, TestClient};
use crate::domain::GlobalStatus;
use axum::{Router, routing::{get, post}, Json, response::IntoResponse, http::StatusCode};
use axum::extract::State;
use std::sync::Arc;

/// HTTP API test handlers
async fn health_handler() -> &'static str { "ok" }

async fn metrics_handler() -> impl IntoResponse {
    (StatusCode::OK, "seata_branch_success_total 0\nseata_branch_failure_total 0\n")
}

#[derive(serde::Deserialize)]
struct StartBody { 
    gid: Option<String>, 
    mode: Option<String>, 
    payload: Option<Vec<u8>> 
}

async fn start_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<StartBody>
) -> impl IntoResponse {
    match state.saga.start(
        body.gid.unwrap_or_default(), 
        body.mode.unwrap_or_default(), 
        body.payload.unwrap_or_default()
    ).await {
        Ok(gid) => (StatusCode::OK, Json(serde_json::json!({"gid": gid}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct GidBody { gid: String }

async fn submit_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<GidBody>
) -> impl IntoResponse {
    match state.saga.submit(body.gid).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn abort_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<GidBody>
) -> impl IntoResponse {
    match state.saga.abort(body.gid).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct BranchBody { 
    gid: String, 
    branch_id: String, 
    action: String 
}

async fn branch_add_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<BranchBody>
) -> impl IntoResponse {
    match state.saga.add_branch(body.gid, body.branch_id, body.action).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct BranchStateBody { 
    gid: String, 
    branch_id: String 
}

async fn branch_succeed_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<BranchStateBody>
) -> impl IntoResponse {
    // barrier confirm idempotency
    match state.barrier.insert(&body.gid, &body.branch_id, crate::domain::BarrierPhase::Confirm).await {
        Ok(false) => return (StatusCode::OK, "").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.branch_succeed(body.gid, body.branch_id).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn branch_try_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<BranchBody>
) -> impl IntoResponse {
    match state.barrier.insert(&body.gid, &body.branch_id, crate::domain::BarrierPhase::Try).await {
        Ok(false) => return (StatusCode::OK, "").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.add_branch(body.gid, body.branch_id, body.action).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn branch_fail_handler(
    State(state): State<Arc<TestServer>>, 
    Json(body): Json<BranchStateBody>
) -> impl IntoResponse {
    // barrier cancel idempotency
    match state.barrier.insert(&body.gid, &body.branch_id, crate::domain::BarrierPhase::Cancel).await {
        Ok(false) => return (StatusCode::OK, "").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.branch_fail(body.gid, body.branch_id).await {
        Ok(()) => (StatusCode::OK, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_tx_handler(
    State(state): State<Arc<TestServer>>, 
    axum::extract::Path(gid): axum::extract::Path<String>
) -> impl IntoResponse {
    match state.saga.get(&gid).await {
        Ok(Some(tx)) => (StatusCode::OK, Json(tx)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn list_tx_handler(
    State(state): State<Arc<TestServer>>, 
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>
) -> impl IntoResponse {
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
    let offset = q.get("offset").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let status = match q.get("status").map(|s| s.as_str()) {
        Some("SUBMITTED") => Some(GlobalStatus::Submitted),
        Some("ABORTING") => Some(GlobalStatus::Aborting),
        Some("ABORTED") => Some(GlobalStatus::Aborted),
        Some("COMMITTING") => Some(GlobalStatus::Committing),
        Some("COMMITTED") => Some(GlobalStatus::Committed),
        _ => None,
    };
    match state.saga.list(limit, offset, status).await {
        Ok(list) => (StatusCode::OK, Json(list)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Create test HTTP server
async fn create_test_server() -> (String, TestClient) {
    let server = TestServer::new().await;
    let state = Arc::new(server);
    
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/api/start", post(start_handler))
        .route("/api/submit", post(submit_handler))
        .route("/api/abort", post(abort_handler))
        .route("/api/branch/add", post(branch_add_handler))
        .route("/api/branch/try", post(branch_try_handler))
        .route("/api/branch/succeed", post(branch_succeed_handler))
        .route("/api/branch/fail", post(branch_fail_handler))
        .route("/api/tx/{gid}", get(get_tx_handler))
        .route("/api/tx", get(list_tx_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}:{}", addr.ip(), addr.port());
    
    tokio::spawn(async move { 
        axum::serve(listener, app).await.unwrap(); 
    });
    
    let client = TestClient::new(base_url.clone());
    (base_url, client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let (_, client) = create_test_server().await;
        
        let response = client.health().await.unwrap();
        assert_eq!(response, "ok");
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let (_, client) = create_test_server().await;
        
        let response = client.metrics().await.unwrap();
        assert!(response.contains("seata_branch_success_total"));
        assert!(response.contains("seata_branch_failure_total"));
    }

    #[tokio::test]
    async fn test_start_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Test with custom GID
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"test payload".to_vec())
        ).await.unwrap();
        
        assert_eq!(response["gid"], "test-gid");
        
        // Test with auto-generated GID
        let response = client.start(None, None, None).await.unwrap();
        assert!(response["gid"].as_str().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_branch_add_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Add a branch
        let response = client.add_branch(
            gid.clone(),
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_submit_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Submit the transaction
        let response = client.submit(gid).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_abort_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Abort the transaction
        let response = client.abort(gid).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_branch_try_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Try a branch
        let response = client.branch_try(
            gid.clone(),
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        
        assert_eq!(response.status(), 200);
        
        // Try again (should be idempotent)
        let response = client.branch_try(
            gid,
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_branch_succeed_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Add a branch first
        client.add_branch(
            gid.clone(),
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        
        // Succeed the branch
        let response = client.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();
        assert_eq!(response.status(), 200);
        
        // Try again (should be idempotent)
        let response = client.branch_succeed(gid, "branch1".to_string()).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_branch_fail_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Add a branch first
        client.add_branch(
            gid.clone(),
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        
        // Fail the branch
        let response = client.branch_fail(gid.clone(), "branch1".to_string()).await.unwrap();
        assert_eq!(response.status(), 200);
        
        // Try again (should be idempotent)
        let response = client.branch_fail(gid, "branch1".to_string()).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_get_tx_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("test-gid".to_string()),
            Some("saga".to_string()),
            Some(b"payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Get the transaction
        let response = client.get_tx(gid.clone()).await.unwrap();
        assert_eq!(response.status(), 200);
        
        let tx: serde_json::Value = response.json().await.unwrap();
        assert_eq!(tx["gid"], gid);
        
        // Get non-existent transaction
        let response = client.get_tx("non-existent".to_string()).await.unwrap();
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_list_tx_endpoint() {
        let (_, client) = create_test_server().await;
        
        // Create multiple transactions
        for i in 0..3 {
            client.start(
                Some(format!("test-gid-{}", i)),
                Some("saga".to_string()),
                Some(b"payload".to_vec())
            ).await.unwrap();
        }
        
        // List all transactions
        let response = client.list_tx(Some(10), Some(0), None).await.unwrap();
        assert_eq!(response.status(), 200);
        
        let txs: Vec<serde_json::Value> = response.json().await.unwrap();
        assert!(txs.len() >= 3);
        
        // List with limit
        let response = client.list_tx(Some(2), Some(0), None).await.unwrap();
        assert_eq!(response.status(), 200);
        
        let txs: Vec<serde_json::Value> = response.json().await.unwrap();
        assert!(txs.len() <= 2);
        
        // List with status filter
        let response = client.list_tx(Some(10), Some(0), Some("SUBMITTED".to_string())).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_full_saga_workflow() {
        let (_, client) = create_test_server().await;
        
        // Start a transaction
        let response = client.start(
            Some("workflow-test".to_string()),
            Some("saga".to_string()),
            Some(b"workflow payload".to_vec())
        ).await.unwrap();
        let gid = response["gid"].as_str().unwrap().to_string();
        
        // Add branches
        client.add_branch(
            gid.clone(),
            "branch1".to_string(),
            "http://example.com/action1".to_string()
        ).await.unwrap();
        
        client.add_branch(
            gid.clone(),
            "branch2".to_string(),
            "http://example.com/action2".to_string()
        ).await.unwrap();
        
        // Set branches to succeed
        client.branch_succeed(gid.clone(), "branch1".to_string()).await.unwrap();
        client.branch_succeed(gid.clone(), "branch2".to_string()).await.unwrap();
        
        // Submit the transaction
        let response = client.submit(gid.clone()).await.unwrap();
        assert_eq!(response.status(), 200);
        
        // Verify the transaction status
        let response = client.get_tx(gid).await.unwrap();
        assert_eq!(response.status(), 200);
        
        let tx: serde_json::Value = response.json().await.unwrap();
        assert_eq!(tx["gid"], "workflow-test");
        assert_eq!(tx["status"], "COMMITTED");
    }

    #[tokio::test]
    async fn test_error_handling() {
        let (_, client) = create_test_server().await;
        
        // Test submitting non-existent transaction
        let response = client.submit("non-existent".to_string()).await.unwrap();
        assert_eq!(response.status(), 200); // Should not error, just do nothing
        
        // Test aborting non-existent transaction
        let response = client.abort("non-existent".to_string()).await.unwrap();
        assert_eq!(response.status(), 200); // Should not error, just do nothing
        
        // Test adding branch to non-existent transaction
        let response = client.add_branch(
            "non-existent".to_string(),
            "branch1".to_string(),
            "http://example.com/action".to_string()
        ).await.unwrap();
        assert_eq!(response.status(), 200); // Should not error, just do nothing
    }
}
