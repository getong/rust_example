#!/bin/bash

# Advanced cluster management script for CrabCluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_BASE_PORT=8080

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a port is available
is_port_available() {
    local port=$1
    ! nc -z localhost $port 2>/dev/null
}

# Function to wait for a node to be ready
wait_for_node() {
    local port=$1
    local timeout=30
    local count=0
    
    log_info "Waiting for node on port $port to be ready..."
    while [ $count -lt $timeout ]; do
        if curl -s http://127.0.0.1:$port/get-id > /dev/null 2>&1; then
            log_success "Node on port $port is ready"
            return 0
        fi
        sleep 1
        count=$((count + 1))
    done
    
    log_error "Node on port $port failed to start within $timeout seconds"
    return 1
}

# Function to get node ID
get_node_id() {
    local port=$1
    curl -s http://127.0.0.1:$port/get-id | jq -r '.'
}

# Function to check if cluster is initialized
is_cluster_initialized() {
    local port=$1
    local result=$(curl -s http://127.0.0.1:$port/metrics | jq -r '.Ok.state // "unknown"')
    [ "$result" != "unknown" ] && [ "$result" != "NonVoter" ]
}

# Function to start a single node
start_node() {
    local port=$1
    local logfile="${SCRIPT_DIR}/logs/node_${port}.log"
    
    mkdir -p "${SCRIPT_DIR}/logs"
    
    if ! is_port_available $port; then
        log_warning "Port $port is already in use"
        return 1
    fi
    
    log_info "Starting node on port $port..."
    nohup cargo run -- --bind-addr 127.0.0.1:$port > "$logfile" 2>&1 &
    echo $! > "${SCRIPT_DIR}/pids/node_${port}.pid"
    
    if wait_for_node $port; then
        log_success "Node started on port $port (PID: $!)"
        return 0
    else
        log_error "Failed to start node on port $port"
        return 1
    fi
}

# Function to stop a node
stop_node() {
    local port=$1
    local pidfile="${SCRIPT_DIR}/pids/node_${port}.pid"
    
    if [ -f "$pidfile" ]; then
        local pid=$(cat "$pidfile")
        if kill -0 $pid 2>/dev/null; then
            log_info "Stopping node on port $port (PID: $pid)..."
            kill $pid
            rm -f "$pidfile"
            log_success "Node on port $port stopped"
        else
            log_warning "Node on port $port was not running"
            rm -f "$pidfile"
        fi
    else
        log_warning "No PID file found for node on port $port"
    fi
}

# Function to initialize cluster
init_cluster() {
    local leader_port=${1:-$DEFAULT_BASE_PORT}
    
    log_info "Initializing cluster on port $leader_port..."
    
    if is_cluster_initialized $leader_port; then
        log_warning "Cluster is already initialized"
        return 0
    fi
    
    local result=$(curl -s http://127.0.0.1:$leader_port/init)
    if echo "$result" | grep -q "Ok"; then
        log_success "Cluster initialized successfully"
        return 0
    else
        log_error "Failed to initialize cluster: $result"
        return 1
    fi
}

# Function to add a node to cluster
add_node_to_cluster() {
    local leader_port=$1
    local new_port=$2
    
    local new_node_id=$(get_node_id $new_port)
    if [ -z "$new_node_id" ] || [ "$new_node_id" = "null" ]; then
        log_error "Could not get node ID for port $new_port"
        return 1
    fi
    
    log_info "Adding node $new_node_id (port $new_port) as learner..."
    local result=$(curl -s -X POST http://127.0.0.1:$leader_port/add-learner \
        -H "Content-Type: application/json" \
        -d "[\"$new_node_id\", \"127.0.0.1:$new_port\"]")
    
    if echo "$result" | grep -q "Ok"; then
        log_success "Node added as learner"
        return 0
    else
        log_error "Failed to add node as learner: $result"
        return 1
    fi
}

# Function to get all cluster members
get_cluster_members() {
    local port=${1:-$DEFAULT_BASE_PORT}
    curl -s http://127.0.0.1:$port/metrics | jq -r '.Ok.membership_config.membership.nodes | keys[]' 2>/dev/null || echo ""
}

# Function to promote nodes to voters
promote_to_voters() {
    local leader_port=${1:-$DEFAULT_BASE_PORT}
    
    log_info "Getting current cluster members..."
    local members=$(get_cluster_members $leader_port)
    
    if [ -z "$members" ]; then
        log_error "Could not get cluster members"
        return 1
    fi
    
    # Build JSON array of member IDs
    local membership_json="["
    local first=true
    for member in $members; do
        if [ "$first" = true ]; then
            first=false
        else
            membership_json+=","
        fi
        membership_json+="\"$member\""
    done
    membership_json+="]"
    
    log_info "Promoting nodes to voting members..."
    log_info "Membership: $membership_json"
    
    local result=$(curl -s -X POST http://127.0.0.1:$leader_port/change-membership \
        -H "Content-Type: application/json" \
        -d "$membership_json")
    
    if echo "$result" | grep -q "Ok"; then
        log_success "All nodes promoted to voting members"
        return 0
    else
        log_error "Failed to change membership: $result"
        return 1
    fi
}

# Function to show cluster status
show_status() {
    local base_port=${1:-$DEFAULT_BASE_PORT}
    
    echo ""
    log_info "=== Cluster Status ==="
    
    # Find running nodes
    local running_nodes=()
    for port in $(seq $base_port $((base_port + 10))); do
        if curl -s http://127.0.0.1:$port/get-id > /dev/null 2>&1; then
            running_nodes+=($port)
        fi
    done
    
    if [ ${#running_nodes[@]} -eq 0 ]; then
        log_warning "No running nodes found"
        return 1
    fi
    
    # Get leader info
    local leader_id=$(curl -s http://127.0.0.1:${running_nodes[0]}/metrics | jq -r '.Ok.current_leader // "unknown"')
    echo "Leader: $leader_id"
    
    echo ""
    echo "Nodes:"
    for port in "${running_nodes[@]}"; do
        local node_id=$(get_node_id $port)
        local state=$(curl -s http://127.0.0.1:$port/metrics | jq -r '.Ok.state // "unknown"')
        local is_leader=""
        if [ "$node_id" = "$leader_id" ]; then
            is_leader=" (LEADER)"
        fi
        echo "  Port $port: $state$is_leader"
        echo "    ID: $node_id"
        echo "    URL: http://127.0.0.1:$port"
    done
    
    echo ""
}

# Function to stop all nodes
stop_all() {
    log_info "Stopping all nodes..."
    mkdir -p "${SCRIPT_DIR}/pids"
    
    for pidfile in "${SCRIPT_DIR}/pids"/*.pid; do
        if [ -f "$pidfile" ]; then
            local port=$(basename "$pidfile" .pid | sed 's/node_//')
            stop_node $port
        fi
    done
    
    log_success "All nodes stopped"
}

# Function to show usage
usage() {
    echo "Usage: $0 COMMAND [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  start-cluster N [BASE_PORT]    Start N-node cluster (default base port: 8080)"
    echo "  add-node PORT [LEADER_PORT]    Add a new node to existing cluster"
    echo "  start-node PORT               Start a single node"
    echo "  stop-node PORT                Stop a single node"
    echo "  stop-all                      Stop all nodes"
    echo "  status [BASE_PORT]            Show cluster status"
    echo "  init [PORT]                   Initialize cluster"
    echo "  promote [LEADER_PORT]         Promote learners to voters"
    echo ""
    echo "Examples:"
    echo "  $0 start-cluster 5            # Start 5-node cluster on ports 8080-8084"
    echo "  $0 start-cluster 3 9000       # Start 3-node cluster on ports 9000-9002"
    echo "  $0 add-node 8085              # Add node on port 8085 to cluster"
    echo "  $0 status                     # Show current cluster status"
}

# Main script logic
case "${1:-}" in
    "start-cluster")
        NODE_COUNT=${2:-3}
        BASE_PORT=${3:-$DEFAULT_BASE_PORT}
        
        log_info "Starting $NODE_COUNT-node cluster on ports $BASE_PORT-$((BASE_PORT + NODE_COUNT - 1))"
        mkdir -p "${SCRIPT_DIR}/pids"
        
        # Start all nodes
        for i in $(seq 0 $((NODE_COUNT - 1))); do
            port=$((BASE_PORT + i))
            start_node $port || exit 1
        done
        
        # Initialize cluster
        sleep 2
        init_cluster $BASE_PORT || exit 1
        
        # Add other nodes
        for i in $(seq 1 $((NODE_COUNT - 1))); do
            port=$((BASE_PORT + i))
            add_node_to_cluster $BASE_PORT $port || exit 1
            sleep 1
        done
        
        # Promote all to voters
        sleep 2
        promote_to_voters $BASE_PORT || exit 1
        
        log_success "Cluster setup complete!"
        show_status $BASE_PORT
        ;;
        
    "add-node")
        NEW_PORT=${2:-}
        LEADER_PORT=${3:-$DEFAULT_BASE_PORT}
        
        if [ -z "$NEW_PORT" ]; then
            log_error "Port number required"
            usage
            exit 1
        fi
        
        start_node $NEW_PORT || exit 1
        sleep 2
        add_node_to_cluster $LEADER_PORT $NEW_PORT || exit 1
        promote_to_voters $LEADER_PORT || exit 1
        ;;
        
    "start-node")
        PORT=${2:-}
        if [ -z "$PORT" ]; then
            log_error "Port number required"
            usage
            exit 1
        fi
        start_node $PORT
        ;;
        
    "stop-node")
        PORT=${2:-}
        if [ -z "$PORT" ]; then
            log_error "Port number required"
            usage
            exit 1
        fi
        stop_node $PORT
        ;;
        
    "stop-all")
        stop_all
        ;;
        
    "status")
        BASE_PORT=${2:-$DEFAULT_BASE_PORT}
        show_status $BASE_PORT
        ;;
        
    "init")
        PORT=${2:-$DEFAULT_BASE_PORT}
        init_cluster $PORT
        ;;
        
    "promote")
        PORT=${2:-$DEFAULT_BASE_PORT}
        promote_to_voters $PORT
        ;;
        
    *)
        usage
        exit 1
        ;;
esac
