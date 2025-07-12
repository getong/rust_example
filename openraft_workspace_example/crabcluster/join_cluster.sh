#!/bin/bash

# Script to demonstrate joining a second node to the crabcluster

echo "=== Starting CrabCluster Join Demo ==="

# Start first node in background
echo "1. Starting first node on 127.0.0.1:8080..."
cargo run -- --bind-addr 127.0.0.1:8080 &
FIRST_PID=$!
sleep 3

# Initialize the cluster
echo "2. Initializing cluster..."
curl -X GET http://127.0.0.1:8080/init
echo ""

# Get first node ID
echo "3. Getting first node ID..."
FIRST_NODE_ID=$(curl -s http://127.0.0.1:8080/get-id | jq -r '.')
echo "First node ID: $FIRST_NODE_ID"

# Start second node in background  
echo "4. Starting second node on 127.0.0.1:8081..."
cargo run -- --bind-addr 127.0.0.1:8081 &
SECOND_PID=$!
sleep 3

# Get second node ID
echo "5. Getting second node ID..."
SECOND_NODE_ID=$(curl -s http://127.0.0.1:8081/get-id | jq -r '.')
echo "Second node ID: $SECOND_NODE_ID"

# Add second node as learner
echo "6. Adding second node as learner..."
curl -X POST http://127.0.0.1:8080/add-learner \
  -H "Content-Type: application/json" \
  -d "[\"$SECOND_NODE_ID\", \"127.0.0.1:8081\"]"
echo ""

# Change membership to include both nodes
echo "7. Promoting second node to voting member..."
curl -X POST http://127.0.0.1:8080/change-membership \
  -H "Content-Type: application/json" \
  -d "[\"$FIRST_NODE_ID\", \"$SECOND_NODE_ID\"]"
echo ""

# Check cluster metrics
echo "8. Checking cluster metrics..."
curl -s http://127.0.0.1:8080/metrics | jq '.'

echo ""
echo "=== Cluster setup complete! ==="
echo "First node: http://127.0.0.1:8080"
echo "Second node: http://127.0.0.1:8081"
echo ""
echo "Press Ctrl+C to stop both nodes"

# Wait for user to stop
trap "kill $FIRST_PID $SECOND_PID 2>/dev/null" EXIT
wait
