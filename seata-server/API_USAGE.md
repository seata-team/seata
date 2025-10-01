# Seata Server API Usage Guide

## Overview

Seata Server provides both HTTP REST API and gRPC API for distributed transaction management. This guide covers the HTTP API usage with practical examples.

## Base URL

```
http://127.0.0.1:36789
```

## Transaction Management

### 1. Start Global Transaction

**Endpoint**: `POST /api/start`

**Request Body**:
```json
{
  "payload": "base64_encoded_data"  // Optional
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
curl -X POST http://127.0.0.1:36789/api/start \
  -H 'content-type: application/json' \
  -d '{}'
```

### 2. Add Branch Transaction (Saga)

**Endpoint**: `POST /api/branch/add`

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id",
  "action": "http://service-endpoint/action"
}
```

**Example**:
```bash
curl -X POST http://127.0.0.1:36789/api/branch/add \
  -H 'content-type: application/json' \
  -d '{
    "gid": "your-transaction-id",
    "branch_id": "b1",
    "action": "http://httpbin.org/status/200"
  }'
```

### 3. Submit Transaction

**Endpoint**: `POST /api/submit`

**Request Body**:
```json
{
  "gid": "transaction-global-id"
}
```

**Example**:
```bash
curl -X POST http://127.0.0.1:36789/api/submit \
  -H 'content-type: application/json' \
  -d '{"gid": "your-transaction-id"}'
```

### 4. Abort Transaction

**Endpoint**: `POST /api/abort`

**Request Body**:
```json
{
  "gid": "transaction-global-id"
}
```

## TCC (Try-Confirm-Cancel) Operations

### 1. Try Phase

**Endpoint**: `POST /api/branch/try`

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id",
  "action": "http://service-endpoint/try",
  "payload": "base64_encoded_data"
}
```

**Example**:
```bash
curl -X POST http://127.0.0.1:36789/api/branch/try \
  -H 'content-type: application/json' \
  -d '{
    "gid": "your-transaction-id",
    "branch_id": "b1",
    "action": "http://httpbin.org/status/200",
    "payload": "dGVzdA=="
  }'
```

### 2. Confirm Phase

**Endpoint**: `POST /api/branch/confirm`

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Example**:
```bash
curl -X POST http://127.0.0.1:36789/api/branch/confirm \
  -H 'content-type: application/json' \
  -d '{
    "gid": "your-transaction-id",
    "branch_id": "b1"
  }'
```

### 3. Cancel Phase

**Endpoint**: `POST /api/branch/cancel`

**Request Body**:
```json
{
  "gid": "transaction-global-id",
  "branch_id": "unique-branch-id"
}
```

**Example**:
```bash
curl -X POST http://127.0.0.1:36789/api/branch/cancel \
  -H 'content-type: application/json' \
  -d '{
    "gid": "your-transaction-id",
    "branch_id": "b1"
  }'
```

## Query Operations

### 1. Get Single Transaction

**Endpoint**: `GET /api/tx/{gid}`

**Example**:
```bash
curl http://127.0.0.1:36789/api/tx/your-transaction-id
```

### 2. List Transactions

**Endpoint**: `GET /api/tx`

**Query Parameters**:
- `limit`: Number of transactions to return (default: 10)
- `offset`: Number of transactions to skip (default: 0)
- `status`: Filter by status (`SUBMITTED`, `COMMITTED`, `ABORTED`)

**Example**:
```bash
# List all transactions
curl "http://127.0.0.1:36789/api/tx"

# List with pagination
curl "http://127.0.0.1:36789/api/tx?limit=5&offset=0"

# Filter by status
curl "http://127.0.0.1:36789/api/tx?status=COMMITTED"
```

## Complete Workflow Examples

### Saga Transaction Example

```bash
# 1. Start transaction
GID=$(curl -s -X POST http://127.0.0.1:36789/api/start \
  -H 'content-type: application/json' \
  -d '{}' | jq -r '.gid')

echo "Transaction ID: $GID"

# 2. Add branches
curl -X POST http://127.0.0.1:36789/api/branch/add \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\"}"

curl -X POST http://127.0.0.1:36789/api/branch/add \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b2\",\"action\":\"http://httpbin.org/status/201\"}"

# 3. Submit transaction
curl -X POST http://127.0.0.1:36789/api/submit \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\"}"

# 4. Check result
curl "http://127.0.0.1:36789/api/tx/$GID"
```

### TCC Transaction Example

```bash
# 1. Start transaction
GID=$(curl -s -X POST http://127.0.0.1:36789/api/start \
  -H 'content-type: application/json' \
  -d '{}' | jq -r '.gid')

echo "Transaction ID: $GID"

# 2. Try phase
curl -X POST http://127.0.0.1:36789/api/branch/try \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\",\"payload\":\"dGVzdA==\"}"

# 3. Confirm phase
curl -X POST http://127.0.0.1:36789/api/branch/confirm \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\"}"

# 4. Check result
curl "http://127.0.0.1:36789/api/tx/$GID"
```

## Response Formats

### Transaction Object

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

### Status Values

**Global Status**:
- `SUBMITTED`: Transaction created, branches prepared
- `COMMITTED`: All branches succeeded
- `ABORTED`: One or more branches failed

**Branch Status**:
- `PREPARED`: Branch added but not executed
- `SUCCEED`: Branch executed successfully
- `FAILED`: Branch execution failed

## Monitoring

### Metrics Endpoint

**Endpoint**: `GET /metrics`

**Example**:
```bash
curl http://127.0.0.1:36789/metrics
```

**Key Metrics**:
- `seata_branch_success_total`: Number of successful branch executions
- `seata_branch_failure_total`: Number of failed branch executions
- `seata_branch_latency_seconds`: Branch execution latency histogram

## Error Handling

All endpoints return appropriate HTTP status codes:
- `200 OK`: Success
- `400 Bad Request`: Invalid request format
- `404 Not Found`: Transaction not found
- `500 Internal Server Error`: Server error

Error responses include a JSON body with error details:

```json
{
  "error": "Error message description"
}
```

## Configuration

The server can be configured via environment variables or `conf.yaml`:

### Environment Variables

```bash
# Storage backend
export SEATA_STORE=redis  # or mysql, postgres, sled

# Redis configuration
export SEATA_REDIS_URL=redis://127.0.0.1/

# MySQL configuration
export SEATA_MYSQL_URL=mysql://user:pass@localhost:3306/seata

# PostgreSQL configuration
export SEATA_POSTGRES_URL=postgres://user:pass@localhost:5432/seata

# Server ports
export SEATA_HTTP_PORT=36789
export SEATA_GRPC_PORT=36790

# Execution settings
export SEATA_REQUEST_TIMEOUT_SECS=30
export SEATA_RETRY_INTERVAL_SECS=1
export SEATA_TIMEOUT_TO_FAIL_SECS=60
export SEATA_BRANCH_PARALLELISM=10
```

### Configuration File

Create `conf.yaml` in the server directory:

```yaml
server:
  http_port: 36789
  grpc_port: 36790

store:
  type: redis
  redis:
    url: redis://127.0.0.1/

exec:
  request_timeout_secs: 30
  retry_interval_secs: 1
  timeout_to_fail_secs: 60
  branch_parallelism: 10
```

## Best Practices

1. **Idempotency**: All operations are idempotent. You can safely retry failed operations.

2. **Concurrent Execution**: The server supports concurrent branch execution. Configure `branch_parallelism` based on your needs.

3. **Timeout Handling**: Set appropriate timeouts for branch execution to avoid hanging transactions.

4. **Monitoring**: Use the metrics endpoint to monitor transaction success rates and latencies.

5. **Error Recovery**: Implement retry logic in your client applications for network failures.

6. **Payload Encoding**: Use base64 encoding for binary payloads in TCC operations.

## Troubleshooting

### Common Issues

1. **Connection Refused**: Ensure the server is running on the correct port.

2. **Transaction Not Found**: Check that the transaction ID is correct and the transaction exists.

3. **Branch Execution Failed**: Verify that the action URLs are accessible and return appropriate HTTP status codes.

4. **Redis Connection Issues**: Check Redis server status and connection URL.

### Debug Commands

```bash
# Check server status
curl http://127.0.0.1:36789/metrics

# List recent transactions
curl "http://127.0.0.1:36789/api/tx?limit=10"

# Check specific transaction
curl "http://127.0.0.1:36789/api/tx/your-transaction-id"
```


