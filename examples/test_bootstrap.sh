#!/bin/bash

# Test bootstrap workflow with examples directory
set -e

CONFIG_DIR="./examples"
MASTER_PORT=8080
SERVER_URL="http://localhost:$MASTER_PORT"
CERTS_DIR="$CONFIG_DIR/certs"
AGENT_CERT_DIR="$CERTS_DIR/agents/agent-01"

echo "=== Pupoxide Bootstrap Workflow Test ==="
echo ""

# 1. Start Master server in background
echo "1. Starting Master server on port $MASTER_PORT..."
cargo run --quiet -- --config "$CONFIG_DIR" master start --port $MASTER_PORT &
MASTER_PID=$!
sleep 2

echo "   Master PID: $MASTER_PID"
echo ""

# 2. List initial pending requests (should be empty)
echo "2. Listing pending requests (should be empty)..."
cargo run --quiet -- --config "$CONFIG_DIR" master list
echo ""

# 3. Submit bootstrap request from agent
echo "3. Agent submitting bootstrap request..."
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" \
  --bootstrap || true
echo ""

# 4. List pending requests (should show agent-01)
echo "4. Listing pending requests (should show agent-01)..."
cargo run --quiet -- --config "$CONFIG_DIR" master list
echo ""

# 5. Admin approves the request
echo "5. Admin approving bootstrap request for agent-01..."
cargo run --quiet -- --config "$CONFIG_DIR" master sign --node agent-01
echo ""

# 6. Agent checks bootstrap status
echo "6. Agent checking bootstrap status (with 10 second timeout)..."
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" \
  --check --check-timeout 10
echo ""

# 7. Verify certificate was created
echo "7. Verifying certificate files..."
if [ -f "$AGENT_CERT_DIR/agent.pem" ]; then
    echo "   ✓ Certificate found: $AGENT_CERT_DIR/agent.pem"
else
    echo "   ✗ Certificate NOT found!"
fi

if [ -f "$AGENT_CERT_DIR/agent.key" ]; then
    echo "   ✓ Private key found: $AGENT_CERT_DIR/agent.key"
else
    echo "   ✗ Private key NOT found!"
fi
echo ""

# Cleanup
echo "8. Cleaning up (stopping Master)..."
kill $MASTER_PID 2>/dev/null || true
wait $MASTER_PID 2>/dev/null || true

echo "=== Test Complete ==="
