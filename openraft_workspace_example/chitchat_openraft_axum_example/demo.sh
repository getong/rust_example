#!/bin/bash

# Chitchat Cluster Demo Script
echo "🚀 Chitchat Cluster Demo with 5 Services"
echo "=========================================="

# Change to the project directory to ensure logs are created in the right place
cd "$(dirname "$0")"
echo "📁 Running from directory: $(pwd)"

# Check if the binary exists
if [ ! -f "../target/release/chitchat_openraft_axum_example" ] && [ ! -f "../target/debug/chitchat_openraft_axum_example" ]; then
    echo "📦 Building the project first..."
    cargo build --release
fi

# Determine which binary to use
BINARY="../target/release/chitchat_openraft_axum_example"
if [ ! -f "$BINARY" ]; then
  BINARY="../target/debug/chitchat_openraft_axum_example"
fi

echo "✅ Using binary: $BINARY"
echo "📂 Logs will be created in: $(pwd)/logs"

# Create logs directory manually as well
mkdir -p logs
echo "📁 Logs directory created/verified"

# Function to cleanup background processes
cleanup() {
    echo ""
    echo "🛑 Shutting down all nodes..."
    jobs -p | xargs kill 2>/dev/null
    exit 0
}

# Set trap to cleanup on script exit
trap cleanup SIGINT SIGTERM EXIT

# Start Node 1 (Searcher) - no seeds since it's the first
echo "🔍 Starting Node 1 (Searcher)..."
$BINARY \
    --listen_addr 127.0.0.1:10001 \
    --gossip_addr 127.0.0.1:11001 \
    --service searcher \
    --shard 1 &

sleep 2

# Start Node 2 (API Gateway) - connects to Node 1
echo "🌐 Starting Node 2 (API Gateway)..."
$BINARY \
    --listen_addr 127.0.0.1:10002 \
    --gossip_addr 127.0.0.1:11002 \
    --service api_gateway \
    --seed 127.0.0.1:11001 &

sleep 2

# Start Node 3 (Data Processor) - connects to Node 1
echo "⚙️  Starting Node 3 (Data Processor)..."
$BINARY \
    --listen_addr 127.0.0.1:10003 \
    --gossip_addr 127.0.0.1:11003 \
    --service data_processor \
    --shard 2 \
    --seed 127.0.0.1:11001 &

sleep 2

# Start Node 4 (Storage) - connects to Node 1
echo "💾 Starting Node 4 (Storage)..."
$BINARY \
    --listen_addr 127.0.0.1:10004 \
    --gossip_addr 127.0.0.1:11004 \
    --service storage \
    --shard 3 \
    --seed 127.0.0.1:11001 &

sleep 2

# Start Node 5 (Analytics) - connects to Node 1
echo "📊 Starting Node 5 (Analytics)..."
$BINARY \
    --listen_addr 127.0.0.1:10005 \
    --gossip_addr 127.0.0.1:11005 \
    --service analytics \
    --shard 4 \
    --seed 127.0.0.1:11001 &

sleep 3

echo ""
echo "✅ All nodes started!"
echo ""
echo "📁 Check logs in: $(pwd)/logs/chitchat_cluster.log.*"
echo ""
echo "🌍 API Endpoints:"
echo "  Node 1 (Searcher):      http://127.0.0.1:10001/members"
echo "  Node 2 (API Gateway):   http://127.0.0.1:10002/members"
echo "  Node 3 (Data Processor): http://127.0.0.1:10003/members"
echo "  Node 4 (Storage):       http://127.0.0.1:10004/members"
echo "  Node 5 (Analytics):     http://127.0.0.1:10005/members"
echo ""
echo "🔄 Try these commands to interact with the cluster:"
echo "  # View all cluster members from any node:"
echo "  curl http://127.0.0.1:10001/members | jq"
echo ""
echo "  # Update a service (change Node 1 to run as LoadBalancer):"
echo "  curl 'http://127.0.0.1:10001/update_service?service_type=load_balancer&host=127.0.0.1:10001'"
echo ""
echo "  # View cluster state:"
echo "  curl http://127.0.0.1:10001/ | jq"
echo ""
echo "🏃 Demo will run until you press Ctrl+C"

# Wait for all background jobs
wait
