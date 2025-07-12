#!/bin/bash

# Script to demonstrate setting up a 5-node CrabCluster

echo "=== Starting CrabCluster 5-Node Setup ==="

# Array to store process IDs
declare -a PIDS=()
declare -a NODE_IDS=()
declare -a PORTS=(8080 8081 8082 8083 8084)

# Function to cleanup processes on exit
cleanup() {
    echo ""
    echo "Cleaning up processes..."
    for pid in "${PIDS[@]}"; do
        kill $pid 2>/dev/null
    done
    exit 0
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Start all nodes
echo "1. Starting 5 nodes..."
for i in "${!PORTS[@]}"; do
    port=${PORTS[$i]}
    echo "   Starting node $((i+1)) on 127.0.0.1:$port..."
    cargo run -- --bind-addr 127.0.0.1:$port &
    PIDS[$i]=$!
    sleep 2
done

echo "   Waiting for all nodes to start..."
sleep 5

# Initialize the cluster on the first node
echo "2. Initializing cluster on first node..."
INIT_RESULT=$(curl -s http://127.0.0.1:8080/init)
echo "   Init result: $INIT_RESULT"

if echo "$INIT_RESULT" | grep -q "NotAllowed"; then
    echo "   Cluster already initialized, continuing..."
else
    echo "   Cluster initialized successfully"
fi

echo ""

# Get all node IDs
echo "3. Getting node IDs..."
for i in "${!PORTS[@]}"; do
    port=${PORTS[$i]}
    node_id=$(curl -s http://127.0.0.1:$port/get-id | jq -r '.')
    NODE_IDS[$i]=$node_id
    echo "   Node $((i+1)) (port $port): $node_id"
done

echo ""

# Add nodes 2-5 as learners
echo "4. Adding nodes as learners..."
for i in {1..4}; do
    port=${PORTS[$i]}
    node_id=${NODE_IDS[$i]}
    echo "   Adding node $((i+1)) as learner..."
    
    RESULT=$(curl -s -X POST http://127.0.0.1:8080/add-learner \
        -H "Content-Type: application/json" \
        -d "[\"$node_id\", \"127.0.0.1:$port\"]")
    
    if echo "$RESULT" | grep -q "Ok"; then
        echo "   ✓ Node $((i+1)) added as learner"
    else
        echo "   ⚠ Node $((i+1)) add result: $RESULT"
    fi
    sleep 1
done

echo ""

# Promote all nodes to voting members
echo "5. Promoting all nodes to voting members..."
MEMBERSHIP_JSON="["
for i in "${!NODE_IDS[@]}"; do
    if [ $i -gt 0 ]; then
        MEMBERSHIP_JSON+=","
    fi
    MEMBERSHIP_JSON+="\"${NODE_IDS[$i]}\""
done
MEMBERSHIP_JSON+="]"

echo "   Membership config: $MEMBERSHIP_JSON"

MEMBERSHIP_RESULT=$(curl -s -X POST http://127.0.0.1:8080/change-membership \
    -H "Content-Type: application/json" \
    -d "$MEMBERSHIP_JSON")

if echo "$MEMBERSHIP_RESULT" | grep -q "Ok"; then
    echo "   ✓ All nodes promoted to voting members"
else
    echo "   ⚠ Membership change result: $MEMBERSHIP_RESULT"
fi

echo ""

# Wait a moment for cluster to stabilize
echo "6. Waiting for cluster to stabilize..."
sleep 3

# Check cluster status
echo "7. Checking cluster status..."
LEADER=$(curl -s http://127.0.0.1:8080/metrics | jq -r '.Ok.current_leader // "unknown"')
echo "   Current leader: $LEADER"

echo "   Node states:"
for i in "${!PORTS[@]}"; do
    port=${PORTS[$i]}
    state=$(curl -s http://127.0.0.1:$port/metrics | jq -r '.Ok.state // "unknown"')
    echo "     Node $((i+1)) (port $port): $state"
done

echo ""

# Test data replication across all nodes
echo "8. Testing data replication..."
TEST_KEY="cluster_test_$(date +%s)"
TEST_VALUE="5-node cluster working at $(date)"

echo "   Writing test data to leader..."
WRITE_RESULT=$(curl -s -X POST http://127.0.0.1:8080/write \
    -H "Content-Type: application/json" \
    -d "{\"Set\": {\"key\": \"$TEST_KEY\", \"value\": \"$TEST_VALUE\"}}")

if echo "$WRITE_RESULT" | grep -q "Ok"; then
    echo "   ✓ Data written successfully"
    
    echo "   Verifying replication on all nodes..."
    sleep 2
    
    for i in "${!PORTS[@]}"; do
        port=${PORTS[$i]}
        read_result=$(curl -s -X POST http://127.0.0.1:$port/read \
            -H "Content-Type: application/json" \
            -d "\"$TEST_KEY\"")
        
        if echo "$read_result" | grep -q "Ok"; then
            echo "     ✓ Node $((i+1)) (port $port): Data replicated"
        else
            echo "     ✗ Node $((i+1)) (port $port): Replication failed"
        fi
    done
else
    echo "   ✗ Write failed: $WRITE_RESULT"
fi

echo ""
echo "=== 5-Node Cluster Setup Complete! ==="
echo ""
echo "Cluster Endpoints:"
for i in "${!PORTS[@]}"; do
    port=${PORTS[$i]}
    echo "  Node $((i+1)): http://127.0.0.1:$port"
done

echo ""
echo "Quick Commands:"
echo "  # Check cluster status:"
echo "  curl -s http://127.0.0.1:8080/metrics | jq '.Ok.state'"
echo ""
echo "  # Write data:"
echo "  curl -X POST http://127.0.0.1:8080/write -H 'Content-Type: application/json' -d '{\"Set\": {\"key\": \"test\", \"value\": \"hello\"}}'"
echo ""
echo "  # Read data from any node:"
echo "  curl -X POST http://127.0.0.1:8081/read -H 'Content-Type: application/json' -d '\"test\"'"
echo ""
echo "Press Ctrl+C to stop all nodes"

# Keep script running
while true; do
    sleep 10
    # Optional: periodic health check
    if ! curl -s http://127.0.0.1:8080/metrics > /dev/null 2>&1; then
        echo "Warning: Leader node may be down"
    fi
done
