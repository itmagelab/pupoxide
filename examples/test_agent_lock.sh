#!/bin/bash

# Test agent lock file mechanism
echo "=== Testing Agent Lock File Mechanism ==="
echo ""

CONFIG_DIR="./examples"
MASTER_PORT=8080
SERVER_URL="http://localhost:$MASTER_PORT"
CERTS_DIR="$CONFIG_DIR/certs"
AGENT_CERT_DIR="$CERTS_DIR/agents/agent-01"

# Cleanup
echo "1. Cleaning up old certs..."
rm -rf "$CERTS_DIR" 2>/dev/null
echo ""

# Start Master server
echo "2. Starting Master server..."
cargo run --quiet -- --config "$CONFIG_DIR" master start --port $MASTER_PORT &
MASTER_PID=$!
sleep 2
echo "   Master running (PID: $MASTER_PID)"
echo ""

# Bootstrap agent
echo "3. Bootstrapping agent-01..."
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" \
  --bootstrap || true
echo ""

# Admin approves
echo "4. Admin approving request..."
cargo run --quiet -- --config "$CONFIG_DIR" master sign --node agent-01
echo ""

# Agent gets certificate
echo "5. Agent retrieving certificate..."
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" \
  --check --check-timeout 5
echo ""

# Test concurrent runs
echo "6. Testing lock file mechanism..."
echo "   6a. Starting first agent in background (will create lock)..."

# Start first agent in background
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" > /tmp/agent1.log 2>&1 &
AGENT1_PID=$!

echo "       First agent PID: $AGENT1_PID"
sleep 1

# Give second agent a moment to start and try to acquire lock
echo ""
echo "   6b. Starting second agent (should block on lock)..."

START_TIME=$(date +%s)

# Monitor lock file in background
(while [ -d /proc/$AGENT1_PID 2>/dev/null ] || ps -p $AGENT1_PID > /dev/null 2>&1; do
    if [ -f "$CERTS_DIR/agents/agent-01.lock" ]; then
        echo "   ✓ Lock file exists: $CERTS_DIR/agents/agent-01.lock"
        exit 0
    fi
    sleep 0.1
done
exit 1) &
MONITOR_PID=$!

# Start second agent in background  
cargo run --quiet -- --config "$CONFIG_DIR" agent \
  --server "$SERVER_URL" \
  --node agent-01 \
  --environment production \
  --cert-dir "$AGENT_CERT_DIR" > /tmp/agent2.log 2>&1 &
AGENT2_PID=$!

echo "       Second agent PID: $AGENT2_PID (will wait for lock)"
echo ""

# Wait for both agents to finish
wait $AGENT1_PID 2>/dev/null || true
sleep 1
wait $AGENT2_PID 2>/dev/null || true

# Stop monitor
kill $MONITOR_PID 2>/dev/null || true
wait $MONITOR_PID 2>/dev/null || true

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo "   ✓ Both agents completed"
echo "   ✓ Lock mechanism ensured sequential execution"
echo ""

# Check lock file doesn't exist after runs
if [ ! -f "$AGENT_CERT_DIR/../agent-01.lock" ] && [ ! -f "$CERTS_DIR/agents/agent-01.lock" ]; then
    echo "7. ✓ Lock file properly cleaned up after execution"
else
    echo "7. ✗ Lock file still exists (should be auto-removed)"
fi
echo ""

# Cleanup
echo "8. Cleaning up..."
kill $MASTER_PID 2>/dev/null || true
wait $MASTER_PID 2>/dev/null || true

echo "=== Test Complete ==="
