#!/bin/bash

# Seata Server API Test Script

set -e

BASE_URL="http://localhost:36789"

echo "🧪 Testing Seata Server API..."

# Test health endpoint
echo "1. Testing health endpoint..."
curl -s "$BASE_URL/health" | jq '.' || echo "Health check failed"

# Test metrics endpoint
echo -e "\n2. Testing metrics endpoint..."
curl -s "$BASE_URL/metrics" | head -20

# Test start transaction
echo -e "\n3. Testing start transaction..."
GID=$(curl -s -X POST "$BASE_URL/api/start" \
  -H 'content-type: application/json' \
  -d '{}' | jq -r '.gid')

echo "Created transaction: $GID"

# Test add branch
echo -e "\n4. Testing add branch..."
curl -s -X POST "$BASE_URL/api/branch/add" \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\"}" | jq '.' || echo "Add branch failed"

# Test get transaction
echo -e "\n5. Testing get transaction..."
curl -s "$BASE_URL/api/tx/$GID" | jq '.'

# Test submit transaction
echo -e "\n6. Testing submit transaction..."
curl -s -X POST "$BASE_URL/api/submit" \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$GID\"}" | jq '.' || echo "Submit failed"

# Wait a moment for processing
sleep 2

# Test get transaction after submit
echo -e "\n7. Testing get transaction after submit..."
curl -s "$BASE_URL/api/tx/$GID" | jq '.'

# Test list transactions
echo -e "\n8. Testing list transactions..."
curl -s "$BASE_URL/api/tx?limit=5" | jq '.'

# Test TCC operations
echo -e "\n9. Testing TCC operations..."
TCC_GID=$(curl -s -X POST "$BASE_URL/api/start" \
  -H 'content-type: application/json' \
  -d '{}' | jq -r '.gid')

echo "Created TCC transaction: $TCC_GID"

# Try phase
curl -s -X POST "$BASE_URL/api/branch/try" \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$TCC_GID\",\"branch_id\":\"b1\",\"action\":\"http://httpbin.org/status/200\",\"payload\":\"dGVzdA==\"}" | jq '.' || echo "Try failed"

# Confirm phase
curl -s -X POST "$BASE_URL/api/branch/confirm" \
  -H 'content-type: application/json' \
  -d "{\"gid\":\"$TCC_GID\",\"branch_id\":\"b1\"}" | jq '.' || echo "Confirm failed"

echo -e "\n✅ API tests completed!"
echo "Check the transaction status: curl -s '$BASE_URL/api/tx/$GID' | jq '.'"
echo "Check the TCC transaction: curl -s '$BASE_URL/api/tx/$TCC_GID' | jq '.'"
