# Seata Server API Reference

Complete API documentation for the Seata Server distributed transaction coordinator.

## 📋 Table of Contents

- [Overview](#overview)
- [Authentication](#authentication)
- [Error Handling](#error-handling)
- [HTTP API](#http-api)
- [gRPC API](#grpc-api)
- [Data Models](#data-models)
- [Examples](#examples)
- [SDKs](#sdks)

## 🌐 Overview

The Seata Server provides both HTTP REST API and gRPC API for distributed transaction management.

### Base URLs
- **HTTP API**: `http://localhost:36789`
- **gRPC API**: `localhost:36790`

### Content Types
- **Request**: `application/json`
- **Response**: `application/json`
- **gRPC**: Protocol Buffers

## 🔐 Authentication

Currently, no authentication is required. For production deployments, implement appropriate authentication mechanisms.

## ⚠️ Error Handling

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | Bad Request - Invalid input |
| 404 | Not Found - Resource not found |
| 500 | Internal Server Error |

### Error Response Format

```json
{
  "error": "Error message description",
  "code": "ERROR_CODE",
  "details": "Additional error details"
}
```

## 🌍 HTTP API

### Transaction Management

#### Start Global Transaction

**Endpoint**: `POST /api/start`

**Description**: Creates a new global transaction.

**Request Body**:
```json
{
  "gid": "optional-transaction-id",
  "mode": "saga",
  "payload": "base64-encoded-data"
}
```

**Response**:
```json
{
  "gid": "transaction-global-id"
}
```

**Example**:
```bash
curl -X POST http://localhost:36789/api/start \
  -H 'content-type: application/json' \
  -d '{
    "mode": "saga",
    "payload": "dGVzdA=="
  }'
```

#### Submit Transaction

**Endpoint**: `POST /api/submit`

**Description**: Submits a global transaction for execution.

**Request Body**:
```json
{
  "gid": "transaction-global-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Abort Transaction

**Endpoint**: `POST /api/abort`

**Description**: Aborts a global transaction.

**Request Body**:
```json
{
  "gid": "transaction-global-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

### Branch Management

#### Add Branch Transaction

**Endpoint**: `POST /api/branch/add`

**Description**: Adds a branch transaction to a global transaction.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id",
  "action": "http://service-endpoint/action"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Branch Try (TCC)

**Endpoint**: `POST /api/branch/try`

**Description**: Executes the try phase of a TCC branch.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id",
  "action": "http://service-endpoint/try",
  "payload": "base64-encoded-data"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Branch Confirm (TCC)

**Endpoint**: `POST /api/branch/confirm`

**Description**: Executes the confirm phase of a TCC branch.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Branch Cancel (TCC)

**Endpoint**: `POST /api/branch/cancel`

**Description**: Executes the cancel phase of a TCC branch.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Branch Succeed

**Endpoint**: `POST /api/branch/succeed`

**Description**: Marks a branch as successful.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

#### Branch Fail

**Endpoint**: `POST /api/branch/fail`

**Description**: Marks a branch as failed.

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Response**:
```json
{
  "status": "success"
}
```

### Query Operations

#### Get Transaction

**Endpoint**: `GET /api/tx/{gid}`

**Description**: Retrieves a specific transaction.

**Path Parameters**:
- `gid` (string): Transaction global ID

**Response**:
```json
{
  "gid": "transaction-global-id",
  "mode": "saga",
  "status": "COMMITTED",
  "payload": [],
  "branches": [
    {
      "branch_id": "b1",
      "action": "http://service-endpoint",
      "status": "SUCCEED"
    }
  ],
  "updated_unix": 1759284924,
  "created_unix": 1759284835
}
```

#### List Transactions

**Endpoint**: `GET /api/tx`

**Description**: Lists transactions with optional filtering and pagination.

**Query Parameters**:
- `limit` (integer, optional): Number of transactions to return (default: 10)
- `offset` (integer, optional): Number of transactions to skip (default: 0)
- `status` (string, optional): Filter by status (`SUBMITTED`, `COMMITTED`, `ABORTED`)

**Response**:
```json
[
  {
    "gid": "transaction-global-id",
    "mode": "saga",
    "status": "COMMITTED",
    "payload": [],
    "branches": [...],
    "updated_unix": 1759284924,
    "created_unix": 1759284835
  }
]
```

### System Operations

#### Health Check

**Endpoint**: `GET /health`

**Description**: Returns the health status of the server.

**Response**:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### Metrics

**Endpoint**: `GET /metrics`

**Description**: Returns Prometheus-compatible metrics.

**Response**: Prometheus metrics format

## 🔌 gRPC API

### Service Definition

```protobuf
service TransactionService {
  rpc StartGlobal(StartGlobalRequest) returns (StartGlobalResponse);
  rpc Submit(SubmitRequest) returns (SubmitResponse);
  rpc Abort(AbortRequest) returns (AbortResponse);
  rpc AddBranch(AddBranchRequest) returns (AddBranchResponse);
  rpc BranchTry(BranchTryRequest) returns (BranchTryResponse);
  rpc BranchSucceed(BranchSucceedRequest) returns (BranchSucceedResponse);
  rpc BranchFail(BranchFailRequest) returns (BranchFailResponse);
  rpc Get(GetRequest) returns (GetResponse);
  rpc List(ListRequest) returns (ListResponse);
}
```

### Message Types

#### StartGlobalRequest
```protobuf
message StartGlobalRequest {
  string gid = 1;
  string mode = 2;
  bytes payload = 3;
}
```

#### StartGlobalResponse
```protobuf
message StartGlobalResponse {
  string gid = 1;
}
```

#### SubmitRequest
```protobuf
message SubmitRequest {
  string gid = 1;
}
```

#### SubmitResponse
```protobuf
message SubmitResponse {
  string status = 1;
}
```

#### AddBranchRequest
```protobuf
message AddBranchRequest {
  string gid = 1;
  string branch_id = 2;
  string action = 3;
}
```

#### AddBranchResponse
```protobuf
message AddBranchResponse {
  string status = 1;
}
```

#### BranchTryRequest
```protobuf
message BranchTryRequest {
  string gid = 1;
  string branch_id = 2;
  string action = 3;
  bytes payload = 4;
}
```

#### BranchTryResponse
```protobuf
message BranchTryResponse {
  string status = 1;
}
```

#### GetRequest
```protobuf
message GetRequest {
  string gid = 1;
}
```

#### GetResponse
```protobuf
message GetResponse {
  GlobalTxn transaction = 1;
}
```

#### ListRequest
```protobuf
message ListRequest {
  int32 limit = 1;
  int32 offset = 2;
  string status = 3;
}
```

#### ListResponse
```protobuf
message ListResponse {
  repeated GlobalTxn transactions = 1;
}
```

### GlobalTxn Message
```protobuf
message GlobalTxn {
  string gid = 1;
  string mode = 2;
  string status = 3;
  bytes payload = 4;
  repeated BranchTxn branches = 5;
  int64 updated_unix = 6;
  int64 created_unix = 7;
}
```

### BranchTxn Message
```protobuf
message BranchTxn {
  string branch_id = 1;
  string action = 2;
  string status = 3;
}
```

## 📊 Data Models

### Global Transaction

```json
{
  "gid": "string",           // Global transaction ID
  "mode": "string",          // Transaction mode (saga, tcc)
  "status": "string",        // Global status
  "payload": "bytes",        // Transaction payload
  "branches": "BranchTxn[]", // Branch transactions
  "updated_unix": "int64",   // Last update timestamp
  "created_unix": "int64"    // Creation timestamp
}
```

### Branch Transaction

```json
{
  "branch_id": "string",     // Branch ID
  "action": "string",        // Action URL
  "status": "string"         // Branch status
}
```

### Status Values

#### Global Status
- `SUBMITTED`: Transaction created, branches prepared
- `COMMITTED`: All branches succeeded
- `ABORTED`: One or more branches failed

#### Branch Status
- `PREPARED`: Branch added but not executed
- `SUCCEED`: Branch executed successfully
- `FAILED`: Branch execution failed

## 💡 Examples

### Complete Saga Workflow

```bash
# 1. Start transaction
GID=$(curl -s -X POST http://localhost:36789/api/start \
  -H 'content-type: application/json' \
  -d '{"mode":"saga","payload":"dGVzdA=="}' | jq -r '.gid')

echo "Transaction ID: $GID"

# 2. Add branches
curl -X POST http://localhost:36789/api/branch/add \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\"}"

curl -X POST http://localhost:36789/api/branch/add \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b2\",\"action\":\"http://httpbin.org/status/201\"}"

# 3. Submit transaction
curl -X POST http://localhost:36789/api/submit \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\"}"

# 4. Check result
curl "http://localhost:36789/api/tx/$GID"
```

### Complete TCC Workflow

```bash
# 1. Start transaction
GID=$(curl -s -X POST http://localhost:36789/api/start \
  -H 'content-type: application/json' \
  -d '{"mode":"tcc"}' | jq -r '.gid')

echo "Transaction ID: $GID"

# 2. Try phase
curl -X POST http://localhost:36789/api/branch/try \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\",\"payload\":\"dGVzdA==\"}"

# 3. Confirm phase
curl -X POST http://localhost:36789/api/branch/confirm \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\"}"

# 4. Check result
curl "http://localhost:36789/api/tx/$GID"
```

### Error Handling Example

```bash
# Create transaction with invalid data
curl -X POST http://localhost:36789/api/start \
  -H 'content-type: application/json' \
  -d '{"invalid": "data"}'

# Response: 400 Bad Request
{
  "error": "Invalid request format",
  "code": "INVALID_REQUEST"
}
```

### Pagination Example

```bash
# List first 5 transactions
curl "http://localhost:36789/api/tx?limit=5"

# List next 5 transactions
curl "http://localhost:36789/api/tx?limit=5&offset=5"

# Filter by status
curl "http://localhost:36789/api/tx?status=COMMITTED"
```

## 🛠️ SDKs

### Rust SDK

```rust
use seata_client::{SeataClient, Transaction, Branch};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SeataClient::new("http://localhost:36789");
    
    // Start transaction
    let tx = client.start_transaction("saga").await?;
    
    // Add branches
    tx.add_branch("b1", "http://service1/action").await?;
    tx.add_branch("b2", "http://service2/action").await?;
    
    // Submit transaction
    tx.submit().await?;
    
    Ok(())
}
```

### Go SDK

```go
package main

import (
    "context"
    "log"
    "github.com/your-org/seata-go-client"
)

func main() {
    client := seata.NewClient("http://localhost:36789")
    
    // Start transaction
    tx, err := client.StartTransaction(context.Background(), "saga")
    if err != nil {
        log.Fatal(err)
    }
    
    // Add branches
    tx.AddBranch("b1", "http://service1/action")
    tx.AddBranch("b2", "http://service2/action")
    
    // Submit transaction
    err = tx.Submit(context.Background())
    if err != nil {
        log.Fatal(err)
    }
}
```

### Python SDK

```python
import asyncio
from seata_client import SeataClient

async def main():
    client = SeataClient("http://localhost:36789")
    
    # Start transaction
    tx = await client.start_transaction("saga")
    
    # Add branches
    await tx.add_branch("b1", "http://service1/action")
    await tx.add_branch("b2", "http://service2/action")
    
    # Submit transaction
    await tx.submit()

if __name__ == "__main__":
    asyncio.run(main())
```

### JavaScript/Node.js SDK

```javascript
const { SeataClient } = require('seata-client');

async function main() {
    const client = new SeataClient('http://localhost:36789');
    
    // Start transaction
    const tx = await client.startTransaction('saga');
    
    // Add branches
    await tx.addBranch('b1', 'http://service1/action');
    await tx.addBranch('b2', 'http://service2/action');
    
    // Submit transaction
    await tx.submit();
}

main().catch(console.error);
```

## 🔍 Monitoring

### Health Check

```bash
curl http://localhost:36789/health
```

### Metrics

```bash
curl http://localhost:36789/metrics
```

### Key Metrics

- `seata_branch_success_total`: Number of successful branch executions
- `seata_branch_failure_total`: Number of failed branch executions
- `seata_branch_latency_seconds`: Branch execution latency histogram
- `seata_active_transactions`: Number of active transactions

## 🚀 Performance

### Benchmarks

- **Transaction Creation**: < 1ms per transaction
- **Branch Execution**: Concurrent processing with configurable limits
- **Throughput**: 1000+ transactions per second
- **Latency**: Sub-second response times for most operations

### Optimization Tips

1. **Use Redis for high-performance scenarios**
2. **Configure appropriate branch parallelism**
3. **Set reasonable timeouts for branch execution**
4. **Monitor memory usage with large payloads**
5. **Use connection pooling for database backends**

## 🔒 Security

### Best Practices

1. **Use HTTPS in production**
2. **Implement authentication and authorization**
3. **Validate all input data**
4. **Use rate limiting**
5. **Enable audit logging**

### Security Considerations

- No authentication by default (add for production)
- Input validation on all endpoints
- Rate limiting for API endpoints
- Secure storage backend configuration

---

**For more information, see the [Complete Documentation](README.md) and [Docker Setup Guide](README-Docker.md).**




