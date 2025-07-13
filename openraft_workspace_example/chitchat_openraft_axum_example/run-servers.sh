#!/bin/bash

# Kill any existing chitchat_openraft_axum_example processes
killall chitchat_openraft_axum_example 2>/dev/null || true

# Build the project in release mode for better performance
echo "Building project in release mode..."
# cargo build --release

echo "Starting chitchat nodes..."

# Start nodes on ports 10001-10005
# First node (10001) will be the seed node
for i in $(seq 10001 10005)
do
    listen_addr="127.0.0.1:$i"
    echo "Starting node on ${listen_addr}"

    if [ $i -eq 10001 ]; then
        # First node - no seed needed
        cargo run --release -- --listen_addr ${listen_addr} --node_id node_$i &
    else
        # Other nodes use the first node as seed
        cargo run --release -- --listen_addr ${listen_addr} --seed 127.0.0.1:10001 --node_id node_$i &
    fi

    # Small delay between starting nodes
    sleep 1
done

echo ""
echo "All nodes started!"
echo "Node endpoints:"
echo "  Node 1: http://127.0.0.1:10001"
echo "  Node 2: http://127.0.0.1:10002"
echo "  Node 3: http://127.0.0.1:10003"
echo "  Node 4: http://127.0.0.1:10004"
echo "  Node 5: http://127.0.0.1:10005"
echo ""
echo "Test the cluster:"
echo "  curl http://127.0.0.1:10001/ | jq"
echo "  curl \"http://127.0.0.1:10001/set_kv?key=test&value=hello\""
echo "  curl http://127.0.0.1:10002/ | jq '.cluster_state'"
echo ""
echo "Press Enter to stop all nodes..."

read
echo "Stopping all nodes..."
kill 0
