use crate::test_utils::{TestServer, TestRepo};
use crate::domain::{GlobalStatus, BarrierPhase};
use crate::barrier::BarrierOps;
use crate::saga::SagaManager;
use seata_proto::txn::transaction_service_server::{TransactionService, TransactionServiceServer};
use seata_proto::txn::{
    StartGlobalRequest, StartGlobalResponse, SubmitRequest, SubmitResponse,
    AbortRequest, AbortResponse, AddBranchRequest, AddBranchResponse,
    BranchTryRequest, BranchTryResponse, BranchStateRequest, BranchStateResponse,
    GetRequest, GetResponse, ListRequest, ListResponse
};
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use anyhow::Result;

/// gRPC service implementation for testing
#[derive(Clone)]
struct TestTxnService {
    saga: Arc<SagaManager<TestRepo>>,
    barrier: Arc<dyn BarrierOps>,
}

#[tonic::async_trait]
impl TransactionService for TestTxnService {
    async fn start_global(&self, req: Request<StartGlobalRequest>) -> Result<Response<StartGlobalResponse>, Status> {
        let r = req.into_inner();
        let gid = self.saga.start(r.gid, r.mode, r.payload).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(StartGlobalResponse { gid }))
    }

    async fn submit(&self, req: Request<SubmitRequest>) -> Result<Response<SubmitResponse>, Status> {
        let gid = req.get_ref().gid.clone();
        self.saga.submit(gid).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(SubmitResponse {}))
    }

    async fn abort(&self, req: Request<AbortRequest>) -> Result<Response<AbortResponse>, Status> {
        let gid = req.get_ref().gid.clone();
        self.saga.abort(gid).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(AbortResponse {}))
    }

    async fn add_branch(&self, req: Request<AddBranchRequest>) -> Result<Response<AddBranchResponse>, Status> {
        let r = req.into_inner();
        self.saga.add_branch(r.gid, r.branch_id, r.action).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(AddBranchResponse {}))
    }

    async fn branch_try(&self, req: Request<BranchTryRequest>) -> Result<Response<BranchTryResponse>, Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Try).await {
            Ok(false) => return Ok(Response::new(BranchTryResponse {})),
            Err(e) => return Err(Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.add_branch(r.gid, r.branch_id, r.action).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(BranchTryResponse {}))
    }

    async fn branch_succeed(&self, req: Request<BranchStateRequest>) -> Result<Response<BranchStateResponse>, Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Confirm).await {
            Ok(false) => return Ok(Response::new(BranchStateResponse {})),
            Err(e) => return Err(Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.branch_succeed(r.gid, r.branch_id).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(BranchStateResponse {}))
    }

    async fn branch_fail(&self, req: Request<BranchStateRequest>) -> Result<Response<BranchStateResponse>, Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Cancel).await {
            Ok(false) => return Ok(Response::new(BranchStateResponse {})),
            Err(e) => return Err(Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.branch_fail(r.gid, r.branch_id).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(BranchStateResponse {}))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let gid = req.get_ref().gid.clone();
        match self.saga.get(&gid).await
            .map_err(|e| Status::internal(e.to_string()))? {
            Some(tx) => Ok(Response::new(GetResponse { 
                txn_json: serde_json::to_vec(&tx).unwrap_or_default() 
            })),
            None => Err(Status::not_found("not found")),
        }
    }

    async fn list(&self, req: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let limit = req.get_ref().limit as usize;
        let offset = req.get_ref().offset as usize;
        let status_str = req.get_ref().status.clone();
        let status = match status_str.as_str() {
            "SUBMITTED" => Some(GlobalStatus::Submitted),
            "ABORTING" => Some(GlobalStatus::Aborting),
            "ABORTED" => Some(GlobalStatus::Aborted),
            "COMMITTING" => Some(GlobalStatus::Committing),
            "COMMITTED" => Some(GlobalStatus::Committed),
            _ => None,
        };
        let list = self.saga.list(limit, offset, status).await
            .map_err(|e| Status::internal(e.to_string()))?;
        let bytes = list.into_iter().map(|t| serde_json::to_vec(&t).unwrap_or_default()).collect();
        Ok(Response::new(ListResponse { txn_json: bytes }))
    }
}

/// Create test gRPC server
async fn create_test_grpc_server() -> String {
    let server = TestServer::new().await;
    let service = TestTxnService {
        saga: server.saga,
        barrier: server.barrier,
    };
    
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Start server in background
    let _server_handle = tokio::spawn(async move {
        Server::builder()
            .add_service(TransactionServiceServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    
    // Give the server time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    
    let endpoint = format!("http://{}:{}", addr.ip(), addr.port());
    endpoint
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::transport::Channel;
    use seata_proto::txn::transaction_service_client::TransactionServiceClient;

    async fn create_client(endpoint: &str) -> TransactionServiceClient<Channel> {
        TransactionServiceClient::connect(endpoint.to_string()).await.unwrap()
    }

    #[tokio::test]
    async fn test_grpc_start_global() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Test with custom GID
        let request = Request::new(StartGlobalRequest {
            gid: "test-grpc-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"test payload".to_vec(),
        });
        
        let response = client.start_global(request).await.unwrap();
        assert_eq!(response.get_ref().gid, "test-grpc-gid");
        
        // Test with auto-generated GID
        let request = Request::new(StartGlobalRequest {
            gid: "".to_string(),
            mode: "saga".to_string(),
            payload: b"test payload".to_vec(),
        });
        
        let response = client.start_global(request).await.unwrap();
        assert!(!response.get_ref().gid.is_empty());
    }

    #[tokio::test]
    async fn test_grpc_submit() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "submit-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"submit test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Submit the transaction
        let submit_request = Request::new(SubmitRequest { gid });
        let _response = client.submit(submit_request).await.unwrap();
        // SubmitResponse is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_abort() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "abort-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"abort test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Abort the transaction
        let abort_request = Request::new(AbortRequest { gid });
        let _response = client.abort(abort_request).await.unwrap();
        // AbortResponse is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_add_branch() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "branch-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"branch test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Add a branch
        let branch_request = Request::new(AddBranchRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        let _response = client.add_branch(branch_request).await.unwrap();
        // AddBranchResponse is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_branch_try() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "try-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"try test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Try a branch
        let try_request = Request::new(BranchTryRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        let _response = client.branch_try(try_request).await.unwrap();
        // BranchTryResponse is empty, just check it's successful
        
        // Try again (should be idempotent)
        let try_request = Request::new(BranchTryRequest {
            gid,
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        let _response = client.branch_try(try_request).await.unwrap();
        // Response is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_branch_succeed() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "succeed-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"succeed test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Add a branch first
        let branch_request = Request::new(AddBranchRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        client.add_branch(branch_request).await.unwrap();
        
        // Succeed the branch
        let succeed_request = Request::new(BranchStateRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
        });
        let _response = client.branch_succeed(succeed_request).await.unwrap();
        // Response is empty, just check it's successful // BranchStateResponse is empty
        
        // Try again (should be idempotent)
        let succeed_request = Request::new(BranchStateRequest {
            gid,
            branch_id: "branch1".to_string(),
        });
        let _response = client.branch_succeed(succeed_request).await.unwrap();
        // Response is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_branch_fail() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "fail-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"fail test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Add a branch first
        let branch_request = Request::new(AddBranchRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        client.add_branch(branch_request).await.unwrap();
        
        // Fail the branch
        let fail_request = Request::new(BranchStateRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
        });
        let _response = client.branch_fail(fail_request).await.unwrap();
        // Response is empty, just check it's successful // BranchStateResponse is empty
        
        // Try again (should be idempotent)
        let fail_request = Request::new(BranchStateRequest {
            gid,
            branch_id: "branch1".to_string(),
        });
        let _response = client.branch_fail(fail_request).await.unwrap();
        // Response is empty, just check it's successful
    }

    #[tokio::test]
    async fn test_grpc_get() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "get-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"get test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Get the transaction
        let get_request = Request::new(GetRequest { gid: gid.clone() });
        let response = client.get(get_request).await.unwrap();
        
        let txn_json = response.get_ref().txn_json.clone();
        let txn: serde_json::Value = serde_json::from_slice(&txn_json).unwrap();
        assert_eq!(txn["gid"], gid);
        
        // Get non-existent transaction
        let get_request = Request::new(GetRequest { gid: "non-existent".to_string() });
        let result = client.get(get_request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_grpc_list() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Create multiple transactions
        for i in 0..3 {
            let start_request = Request::new(StartGlobalRequest {
                gid: format!("list-test-gid-{}", i),
                mode: "saga".to_string(),
                payload: format!("list test payload {}", i).as_bytes().to_vec(),
            });
            client.start_global(start_request).await.unwrap();
        }
        
        // List all transactions
        let list_request = Request::new(ListRequest {
            limit: 10,
            offset: 0,
            status: "".to_string(),
        });
        let response = client.list(list_request).await.unwrap();
        
        let txn_jsons = response.get_ref().txn_json.clone();
        assert!(txn_jsons.len() >= 3);
        
        // List with limit
        let list_request = Request::new(ListRequest {
            limit: 2,
            offset: 0,
            status: "".to_string(),
        });
        let response = client.list(list_request).await.unwrap();
        
        let txn_jsons = response.get_ref().txn_json.clone();
        assert!(txn_jsons.len() <= 2);
        
        // List with status filter
        let list_request = Request::new(ListRequest {
            limit: 10,
            offset: 0,
            status: "SUBMITTED".to_string(),
        });
        let response = client.list(list_request).await.unwrap();
        
        let txn_jsons = response.get_ref().txn_json.clone();
        assert!(txn_jsons.len() >= 3);
    }

    #[tokio::test]
    async fn test_grpc_full_workflow() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Start a transaction
        let start_request = Request::new(StartGlobalRequest {
            gid: "workflow-test-gid".to_string(),
            mode: "saga".to_string(),
            payload: b"workflow test payload".to_vec(),
        });
        let start_response = client.start_global(start_request).await.unwrap();
        let gid = start_response.get_ref().gid.clone();
        
        // Add branches
        let branch1_request = Request::new(AddBranchRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action1".to_string(),
        });
        client.add_branch(branch1_request).await.unwrap();
        
        let branch2_request = Request::new(AddBranchRequest {
            gid: gid.clone(),
            branch_id: "branch2".to_string(),
            action: "http://example.com/action2".to_string(),
        });
        client.add_branch(branch2_request).await.unwrap();
        
        // Set branches to succeed
        let succeed1_request = Request::new(BranchStateRequest {
            gid: gid.clone(),
            branch_id: "branch1".to_string(),
        });
        client.branch_succeed(succeed1_request).await.unwrap();
        
        let succeed2_request = Request::new(BranchStateRequest {
            gid: gid.clone(),
            branch_id: "branch2".to_string(),
        });
        client.branch_succeed(succeed2_request).await.unwrap();
        
        // Submit the transaction
        let submit_request = Request::new(SubmitRequest { gid: gid.clone() });
        client.submit(submit_request).await.unwrap();
        
        // Verify the transaction
        let get_request = Request::new(GetRequest { gid });
        let response = client.get(get_request).await.unwrap();
        
        let txn_json = response.get_ref().txn_json.clone();
        let txn: serde_json::Value = serde_json::from_slice(&txn_json).unwrap();
        assert_eq!(txn["gid"], "workflow-test-gid");
        assert_eq!(txn["status"], "COMMITTED");
    }

    #[tokio::test]
    async fn test_grpc_error_handling() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Test submitting non-existent transaction
        let submit_request = Request::new(SubmitRequest { gid: "non-existent".to_string() });
        let result = client.submit(submit_request).await;
        assert!(result.is_ok()); // Should not error, just do nothing
        
        // Test aborting non-existent transaction
        let abort_request = Request::new(AbortRequest { gid: "non-existent".to_string() });
        let result = client.abort(abort_request).await;
        assert!(result.is_ok()); // Should not error, just do nothing
        
        // Test adding branch to non-existent transaction
        let branch_request = Request::new(AddBranchRequest {
            gid: "non-existent".to_string(),
            branch_id: "branch1".to_string(),
            action: "http://example.com/action".to_string(),
        });
        let result = client.add_branch(branch_request).await;
        assert!(result.is_ok()); // Should not error, just do nothing
    }

    #[tokio::test]
    async fn test_grpc_concurrent_operations() {
        let endpoint = create_test_grpc_server().await;
        let mut client = create_client(&endpoint).await;
        
        // Create multiple transactions concurrently
        let mut handles = Vec::new();
        for i in 0..5 {
            let mut client = client.clone();
            let handle = tokio::spawn(async move {
                let start_request = Request::new(StartGlobalRequest {
                    gid: format!("concurrent-grpc-gid-{}", i),
                    mode: "saga".to_string(),
                    payload: format!("concurrent payload {}", i).as_bytes().to_vec(),
                });
                client.start_global(start_request).await
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify all transactions exist
        let list_request = Request::new(ListRequest {
            limit: 10,
            offset: 0,
            status: "".to_string(),
        });
        let response = client.list(list_request).await.unwrap();
        
        let txn_jsons = response.get_ref().txn_json.clone();
        assert!(txn_jsons.len() >= 5);
    }
}
