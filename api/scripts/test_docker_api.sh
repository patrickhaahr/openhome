#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:8000}"
API_KEY="${API_KEY:-secret}"
CONTAINER_NAME="${CONTAINER_NAME:-hello-test}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ PASS${NC}: $1"; }
fail() { echo -e "${RED}✗ FAIL${NC}: $1"; exit 1; }
info() { echo -e "${YELLOW}→${NC} $1"; }

auth_header=(-H "Authorization: Bearer $API_KEY")

echo "=== Docker API Endpoint Tests ==="
echo "API URL: $API_URL"
echo "API Key: ${API_KEY:0:4}..."
echo ""

info "Testing health endpoint..."
if curl -s "${auth_header[@]}" "$API_URL/api/health" | grep -q '"status":"ok"'; then
    pass "Health check"
else
    fail "Health check"
fi
echo ""

info "Listing containers..."
response=$(curl -s "${auth_header[@]}" "$API_URL/api/docker")
if echo "$response" | grep -q '"containers"'; then
    pass "List containers"
    container_count=$(echo "$response" | grep -o '"name"' | wc -l)
    echo "  Found $container_count containers"
else
    fail "List containers"
fi
echo ""

info "Getting container details: $CONTAINER_NAME..."
response=$(curl -s "${auth_header[@]}" "$API_URL/api/docker/$CONTAINER_NAME")
if echo "$response" | grep -q '"name":"'"$CONTAINER_NAME"'"'; then
    pass "Get container details"
else
    fail "Get container details"
fi
echo ""

info "Getting container logs..."
response=$(curl -s "${auth_header[@]}" "$API_URL/api/docker/$CONTAINER_NAME/logs")
if [ -n "$response" ]; then
    pass "Get container logs"
else
    fail "Get container logs"
fi
echo ""

info "Getting logs with tail=5..."
response=$(curl -s "${auth_header[@]}" "$API_URL/api/docker/$CONTAINER_NAME/logs?tail=5")
line_count=$(echo "$response" | wc -l)
if [ "$line_count" -le 5 ]; then
    pass "Logs tail parameter ($line_count lines)"
else
    fail "Logs tail parameter (got $line_count lines)"
fi
echo ""

info "Restarting container..."
response=$(curl -s -X POST "${auth_header[@]}" -H "Content-Type: application/json" -d '{"timeout_seconds": 10}' "$API_URL/api/docker/$CONTAINER_NAME/restart")
if echo "$response" | grep -q '"success":true'; then
    pass "Restart container"
else
    fail "Restart container"
fi
echo ""

info "Testing non-existent container (should return 404)..."
response=$(curl -s -w "\n%{http_code}" "${auth_header[@]}" "$API_URL/api/docker/nonexistent-container-12345")
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')
if [ "$http_code" = "500" ]; then
    pass "Non-existent container returns error"
else
    info "Got HTTP $http_code (expected 500 for error)"
fi
echo ""

echo "=== All tests completed ==="
