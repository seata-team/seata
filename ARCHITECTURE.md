# Seata Server Architecture

This document describes the architecture, design decisions, and implementation details of the Seata Server distributed transaction coordinator.

## 📋 Table of Contents

- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Core Components](#core-components)
- [Transaction Patterns](#transaction-patterns)
- [Storage Architecture](#storage-architecture)
- [API Design](#api-design)
- [Concurrency Model](#concurrency-model)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)
- [Security Architecture](#security-architecture)

## 🌐 Overview

Seata Server is a distributed transaction coordinator implemented in Rust that provides Saga and TCC transaction patterns. It's designed for high performance, reliability, and ease of use in microservices architectures.

### Key Design Principles

1. **High Performance**: Optimized for low latency and high throughput
2. **Reliability**: Strong consistency guarantees and fault tolerance
3. **Simplicity**: Easy to understand and integrate
4. **Extensibility**: Pluggable storage backends and transaction patterns
5. **Observability**: Comprehensive monitoring and logging

## 🏗️ System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Client Applications                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │   HTTP      │  │   gRPC      │  │   Admin     │            │
│  │   Client    │  │   Client    │  │   UI        │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Seata Server                              │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                API Layer                                   ││
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        ││
│  │  │   HTTP      │  │   gRPC      │  │   Metrics   │        ││
│  │  │   Server    │  │   Server    │  │   Server    │        ││
│  │  └─────────────┘  └─────────────┘  └─────────────┘        ││
│  └─────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              Transaction Management                         ││
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        ││
│  │  │   Saga      │  │   TCC       │  │   Barrier   │        ││
│  │  │   Manager   │  │   Manager   │  │   Manager   │        ││
│  │  └─────────────┘  └─────────────┘  └─────────────┘        ││
│  └─────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                Storage Layer                               ││
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        ││
│  │  │   Redis     │  │   MySQL     │  │ PostgreSQL  │        ││
│  │  │   Store     │  │   Store     │  │   Store     │        ││
│  │  └─────────────┘  └─────────────┘  └─────────────┘        ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### Component Interaction Flow

```
Client Request → API Layer → Transaction Manager → Storage Layer
     ↓              ↓              ↓              ↓
Response ← API Layer ← Transaction Manager ← Storage Layer
```

## 🔧 Core Components

### 1. API Layer

The API layer provides multiple interfaces for client interaction:

#### HTTP Server (Axum)
- **Port**: 36789
- **Features**: RESTful API, JSON serialization, CORS support
- **Endpoints**: Transaction management, branch operations, queries

#### gRPC Server (Tonic)
- **Port**: 36790
- **Features**: High-performance binary protocol, streaming support
- **Services**: TransactionService with full CRUD operations

#### Metrics Server
- **Port**: 36789/metrics
- **Features**: Prometheus-compatible metrics
- **Metrics**: Success rates, latencies, active transactions

### 2. Transaction Management

#### Saga Manager
```rust
pub struct SagaManager {
    repo: Arc<dyn TxnRepo>,
    exec_config: ExecConfig,
    metrics: Arc<PrometheusRegistry>,
}
```

**Responsibilities**:
- Global transaction lifecycle management
- Branch execution coordination
- Concurrent branch processing
- Status tracking and updates

#### TCC Manager
```rust
pub struct TccManager {
    repo: Arc<dyn TxnRepo>,
    barrier: Arc<dyn BarrierOps>,
}
```

**Responsibilities**:
- Try-Confirm-Cancel pattern implementation
- Idempotency guarantees
- Phase management

#### Barrier Manager
```rust
pub struct Barrier {
    kv: Arc<dyn KV>,
}
```

**Responsibilities**:
- Idempotency barrier implementation
- Phase tracking (try, confirm, cancel)
- Duplicate operation prevention

### 3. Storage Layer

#### Repository Pattern
```rust
#[async_trait]
pub trait TxnRepo {
    async fn save(&self, tx: &GlobalTxn) -> Result<()>;
    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>>;
    async fn list(&self, limit: usize, offset: usize, status: Option<GlobalStatus>) -> Result<Vec<GlobalTxn>>;
}
```

#### Storage Backends

**Redis Store**:
- High-performance in-memory storage
- Suitable for high-throughput scenarios
- Optional persistence with AOF/RDB

**MySQL Store**:
- ACID-compliant persistent storage
- Suitable for critical business data
- Full SQL query capabilities

**PostgreSQL Store**:
- Advanced features (JSON, arrays, etc.)
- Excellent concurrency support
- Rich data types

**Sled Store**:
- Embedded database for development
- Zero-configuration setup
- File-based persistence

## 🔄 Transaction Patterns

### Saga Pattern

The Saga pattern manages distributed transactions through a sequence of local transactions.

#### Saga Flow
```
Start → Add Branches → Submit → Execute Branches → Commit/Abort
  ↓         ↓           ↓           ↓              ↓
Create   Prepare    Validate    HTTP Calls    Update Status
```

#### Implementation
```rust
impl SagaManager {
    pub async fn start(&self, gid: String, mode: String, payload: Vec<u8>) -> Result<String> {
        let tx = GlobalTxn::new(gid, mode, payload, now());
        self.repo.save(&tx).await?;
        Ok(tx.gid)
    }
    
    pub async fn submit(&self, gid: String) -> Result<()> {
        let mut tx = self.repo.load(&gid).await?.unwrap();
        
        // Execute branches concurrently
        let semaphore = Arc::new(Semaphore::new(self.exec_config.branch_parallelism));
        let mut handles = Vec::new();
        
        for branch in &tx.branches {
            let semaphore = semaphore.clone();
            let client = self.http_client.clone();
            let branch = branch.clone();
            
            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                self.execute_branch(&branch).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all branches to complete
        for handle in handles {
            let result = handle.await.unwrap();
            // Update branch status based on result
        }
        
        // Update global transaction status
        self.repo.save(&tx).await?;
        Ok(())
    }
}
```

### TCC Pattern

The TCC pattern uses three phases: Try, Confirm, Cancel.

#### TCC Flow
```
Start → Try Phase → Confirm/Cancel → Update Status
  ↓        ↓           ↓              ↓
Create   Reserve    Finalize      Complete
```

#### Implementation
```rust
impl TccManager {
    pub async fn try(&self, gid: String, branch_id: String, action: String, payload: Vec<u8>) -> Result<()> {
        // Check barrier for idempotency
        if self.barrier.exists(&gid, &branch_id, BarrierPhase::Try).await? {
            return Ok(());
        }
        
        // Execute try phase
        let result = self.execute_action(&action, &payload).await?;
        
        // Record barrier
        self.barrier.insert(&gid, &branch_id, BarrierPhase::Try).await?;
        
        Ok(())
    }
    
    pub async fn confirm(&self, gid: String, branch_id: String) -> Result<()> {
        // Check barrier for idempotency
        if self.barrier.exists(&gid, &branch_id, BarrierPhase::Confirm).await? {
            return Ok(());
        }
        
        // Execute confirm phase
        let result = self.execute_confirm(&gid, &branch_id).await?;
        
        // Record barrier
        self.barrier.insert(&gid, &branch_id, BarrierPhase::Confirm).await?;
        
        Ok(())
    }
}
```

## 💾 Storage Architecture

### Data Models

#### Global Transaction
```rust
pub struct GlobalTxn {
    pub gid: String,
    pub mode: String,
    pub status: GlobalStatus,
    pub payload: Vec<u8>,
    pub branches: Vec<BranchTxn>,
    pub updated_unix: i64,
    pub created_unix: i64,
}
```

#### Branch Transaction
```rust
pub struct BranchTxn {
    pub branch_id: String,
    pub action: String,
    pub status: BranchStatus,
}
```

#### Barrier Record
```rust
pub struct BarrierRecord {
    pub gid: String,
    pub branch_id: String,
    pub phase: BarrierPhase,
    pub created_at: i64,
}
```

### Storage Backend Implementation

#### Redis Implementation
```rust
pub struct RedisTxnRepo {
    client: redis::Client,
}

#[async_trait]
impl TxnRepo for RedisTxnRepo {
    async fn save(&self, tx: &GlobalTxn) -> Result<()> {
        let key = format!("gid:{}", tx.gid);
        let value = serde_json::to_vec(tx)?;
        self.client.set(key, value).await?;
        Ok(())
    }
    
    async fn load(&self, gid: &str) -> Result<Option<GlobalTxn>> {
        let key = format!("gid:{}", gid);
        let value: Option<Vec<u8>> = self.client.get(key).await?;
        match value {
            Some(data) => Ok(Some(serde_json::from_slice(&data)?)),
            None => Ok(None),
        }
    }
}
```

#### MySQL Implementation
```rust
pub struct MysqlTxnRepo {
    pool: sqlx::MySqlPool,
}

#[async_trait]
impl TxnRepo for MysqlTxnRepo {
    async fn save(&self, tx: &GlobalTxn) -> Result<()> {
        sqlx::query!(
            "INSERT INTO global_txns (gid, mode, status, payload, branches, updated_unix, created_unix) 
             VALUES (?, ?, ?, ?, ?, ?, ?) 
             ON DUPLICATE KEY UPDATE 
             mode = VALUES(mode), status = VALUES(status), payload = VALUES(payload), 
             branches = VALUES(branches), updated_unix = VALUES(updated_unix)",
            tx.gid, tx.mode, tx.status, tx.payload, 
            serde_json::to_string(&tx.branches)?, tx.updated_unix, tx.created_unix
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
```

## 🌐 API Design

### RESTful Design Principles

1. **Resource-based URLs**: `/api/tx/{gid}`
2. **HTTP methods**: GET, POST, PUT, DELETE
3. **Status codes**: Appropriate HTTP status codes
4. **Content negotiation**: JSON for all responses
5. **Error handling**: Consistent error response format

### API Endpoints

#### Transaction Management
- `POST /api/start` - Create transaction
- `POST /api/submit` - Submit transaction
- `POST /api/abort` - Abort transaction
- `GET /api/tx/{gid}` - Get transaction
- `GET /api/tx` - List transactions

#### Branch Management
- `POST /api/branch/add` - Add branch
- `POST /api/branch/try` - TCC try phase
- `POST /api/branch/confirm` - TCC confirm phase
- `POST /api/branch/cancel` - TCC cancel phase
- `POST /api/branch/succeed` - Mark branch success
- `POST /api/branch/fail` - Mark branch failure

#### System Operations
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

### gRPC Service Design

```protobuf
service TransactionService {
  // Transaction management
  rpc StartGlobal(StartGlobalRequest) returns (StartGlobalResponse);
  rpc Submit(SubmitRequest) returns (SubmitResponse);
  rpc Abort(AbortRequest) returns (AbortResponse);
  
  // Branch management
  rpc AddBranch(AddBranchRequest) returns (AddBranchResponse);
  rpc BranchTry(BranchTryRequest) returns (BranchTryResponse);
  rpc BranchSucceed(BranchSucceedRequest) returns (BranchSucceedResponse);
  rpc BranchFail(BranchFailRequest) returns (BranchFailResponse);
  
  // Query operations
  rpc Get(GetRequest) returns (GetResponse);
  rpc List(ListRequest) returns (ListResponse);
}
```

## ⚡ Concurrency Model

### Async/Await Architecture

The entire system is built on Rust's async/await model using Tokio runtime:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let repo = Arc::new(create_repo().await?);
    let saga = Arc::new(SagaManager::new(repo));
    
    // Start HTTP server
    let http_server = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await
    });
    
    // Start gRPC server
    let grpc_server = tokio::spawn(async move {
        Server::builder()
            .add_service(TransactionServiceServer::new(service))
            .serve(addr)
            .await
    });
    
    // Wait for both servers
    tokio::try_join!(http_server, grpc_server)?;
    Ok(())
}
```

### Concurrent Branch Execution

```rust
impl SagaManager {
    pub async fn submit(&self, gid: String) -> Result<()> {
        let tx = self.repo.load(&gid).await?.unwrap();
        
        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(self.exec_config.branch_parallelism));
        let mut handles = Vec::new();
        
        for branch in &tx.branches {
            let semaphore = semaphore.clone();
            let client = self.http_client.clone();
            let branch = branch.clone();
            
            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                self.execute_branch(&branch).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all branches to complete
        let results = futures::future::join_all(handles).await;
        
        // Process results and update transaction status
        self.process_results(&mut tx, results).await?;
        
        Ok(())
    }
}
```

### Thread Safety

All components are designed to be thread-safe:

- **Arc<T>**: Reference counting for shared ownership
- **Mutex<T>**: Mutual exclusion for mutable data
- **RwLock<T>**: Reader-writer locks for read-heavy workloads
- **Atomic types**: Lock-free operations where possible

## ⚠️ Error Handling

### Error Types

```rust
#[derive(thiserror::Error, Debug)]
pub enum SeataError {
    #[error("Transaction not found: {gid}")]
    TransactionNotFound { gid: String },
    
    #[error("Branch not found: {branch_id}")]
    BranchNotFound { branch_id: String },
    
    #[error("Invalid status transition: {from} -> {to}")]
    InvalidStatusTransition { from: String, to: String },
    
    #[error("Storage error: {0}")]
    StorageError(#[from] Box<dyn std::error::Error + Send + Sync>),
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}
```

### Error Propagation

```rust
pub async fn execute_branch(&self, branch: &BranchTxn) -> Result<BranchResult> {
    let client = reqwest::Client::new();
    let response = client
        .post(&branch.action)
        .timeout(Duration::from_secs(self.exec_config.request_timeout_secs))
        .send()
        .await
        .map_err(SeataError::HttpError)?;
    
    if response.status().is_success() {
        Ok(BranchResult::Success)
    } else {
        Ok(BranchResult::Failure)
    }
}
```

### Retry Logic

```rust
impl SagaManager {
    pub async fn execute_branch_with_retry(&self, branch: &BranchTxn) -> Result<BranchResult> {
        let mut attempts = 0;
        let max_attempts = self.exec_config.max_retries;
        
        loop {
            match self.execute_branch(branch).await {
                Ok(result) => return Ok(result),
                Err(e) if attempts < max_attempts => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(self.exec_config.retry_interval_secs)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

## 🚀 Performance Considerations

### Memory Management

1. **Zero-copy deserialization**: Using `serde_json::from_slice`
2. **Efficient string handling**: Avoiding unnecessary allocations
3. **Connection pooling**: Reusing HTTP connections
4. **Streaming responses**: For large result sets

### Database Optimization

1. **Indexed queries**: Proper indexing on frequently queried fields
2. **Batch operations**: Grouping multiple operations
3. **Connection pooling**: Managing database connections efficiently
4. **Query optimization**: Using prepared statements

### Caching Strategy

1. **In-memory caching**: For frequently accessed data
2. **Redis caching**: For distributed caching
3. **Cache invalidation**: Proper cache management
4. **Cache warming**: Preloading critical data

### Monitoring and Metrics

```rust
pub struct Metrics {
    pub branch_success_total: Counter,
    pub branch_failure_total: Counter,
    pub branch_latency_seconds: Histogram,
    pub active_transactions: Gauge,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            branch_success_total: Counter::new("seata_branch_success_total", "Successful branch executions")
                .unwrap(),
            branch_failure_total: Counter::new("seata_branch_failure_total", "Failed branch executions")
                .unwrap(),
            branch_latency_seconds: Histogram::new("seata_branch_latency_seconds", "Branch execution latency")
                .unwrap(),
            active_transactions: Gauge::new("seata_active_transactions", "Active transactions")
                .unwrap(),
        }
    }
}
```

## 🔒 Security Architecture

### Input Validation

```rust
pub fn validate_gid(gid: &str) -> Result<()> {
    if gid.is_empty() || gid.len() > 64 {
        return Err(SeataError::InvalidInput("Invalid GID".to_string()));
    }
    
    if !gid.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(SeataError::InvalidInput("GID contains invalid characters".to_string()));
    }
    
    Ok(())
}
```

### Rate Limiting

```rust
pub struct RateLimiter {
    limiter: Arc<Mutex<HashMap<String, TokenBucket>>>,
}

impl RateLimiter {
    pub async fn check_rate_limit(&self, key: &str) -> Result<()> {
        let mut limiter = self.limiter.lock().await;
        let bucket = limiter.entry(key.to_string()).or_insert_with(|| {
            TokenBucket::new(100, Duration::from_secs(60))
        });
        
        if bucket.try_consume(1) {
            Ok(())
        } else {
            Err(SeataError::RateLimitExceeded)
        }
    }
}
```

### Audit Logging

```rust
pub struct AuditLogger {
    logger: Logger,
}

impl AuditLogger {
    pub fn log_transaction_start(&self, gid: &str, user: &str) {
        info!(
            gid = gid,
            user = user,
            action = "transaction_start",
            "Transaction started"
        );
    }
    
    pub fn log_transaction_submit(&self, gid: &str, user: &str, status: &str) {
        info!(
            gid = gid,
            user = user,
            action = "transaction_submit",
            status = status,
            "Transaction submitted"
        );
    }
}
```

## 📊 Monitoring and Observability

### Health Checks

```rust
pub async fn health_check() -> Result<Json<HealthResponse>, StatusCode> {
    // Check database connectivity
    let db_healthy = check_database_health().await.is_ok();
    
    // Check Redis connectivity
    let redis_healthy = check_redis_health().await.is_ok();
    
    let status = if db_healthy && redis_healthy {
        "healthy"
    } else {
        "unhealthy"
    };
    
    Ok(Json(HealthResponse {
        status: status.to_string(),
        timestamp: chrono::Utc::now(),
        database: db_healthy,
        redis: redis_healthy,
    }))
}
```

### Metrics Collection

```rust
pub struct MetricsCollector {
    metrics: Arc<Metrics>,
}

impl MetricsCollector {
    pub fn record_branch_execution(&self, success: bool, duration: Duration) {
        if success {
            self.metrics.branch_success_total.inc();
        } else {
            self.metrics.branch_failure_total.inc();
        }
        
        self.metrics.branch_latency_seconds.observe(duration.as_secs_f64());
    }
}
```

### Distributed Tracing

```rust
pub async fn execute_branch_with_tracing(&self, branch: &BranchTxn) -> Result<BranchResult> {
    let span = tracing::info_span!("execute_branch", branch_id = &branch.branch_id);
    let _enter = span.enter();
    
    tracing::info!("Starting branch execution");
    
    let result = self.execute_branch(branch).await;
    
    match &result {
        Ok(BranchResult::Success) => tracing::info!("Branch execution successful"),
        Ok(BranchResult::Failure) => tracing::warn!("Branch execution failed"),
        Err(e) => tracing::error!(error = %e, "Branch execution error"),
    }
    
    result
}
```

## 🔄 Deployment Architecture

### Container Architecture

```dockerfile
# Multi-stage build for production
FROM rust:1.75-slim as builder
# ... build steps ...

FROM debian:bookworm-slim as runtime
# ... runtime setup ...
```

### Service Discovery

```yaml
# docker-compose.yml
services:
  seata-server:
    image: seata-server:latest
    ports:
      - "36789:36789"
      - "36790:36790"
    environment:
      - SEATA_STORE=redis
      - SEATA_REDIS_URL=redis://redis:6379/
    depends_on:
      - redis
      - mysql
      - postgres
```

### Load Balancing

```nginx
upstream seata_backend {
    server seata-1:36789;
    server seata-2:36789;
    server seata-3:36789;
}

server {
    listen 80;
    location / {
        proxy_pass http://seata_backend;
    }
}
```

## 🎯 Future Enhancements

### Planned Features

1. **XA Transaction Support**: Full XA transaction coordination
2. **Event Sourcing**: Event-driven transaction processing
3. **Multi-Region Support**: Cross-region transaction coordination
4. **Advanced Monitoring**: Real-time transaction visualization
5. **SDK Generation**: Automatic client SDK generation

### Scalability Improvements

1. **Horizontal Scaling**: Multi-instance coordination
2. **Partitioning**: Transaction data partitioning
3. **Caching**: Advanced caching strategies
4. **Streaming**: Real-time transaction streaming

---

This architecture document provides a comprehensive overview of the Seata Server design and implementation. For more details, see the [API Reference](API_REFERENCE.md) and [Complete Documentation](README.md).




