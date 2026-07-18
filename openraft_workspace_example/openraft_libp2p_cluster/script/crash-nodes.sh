#!/usr/bin/env bash
# Stop selected nodes of a running run-20nodes.sh cluster to exercise the
# self-healing paths (voter replacement, learner re-join).
#
# Usage:
#   DB_ROOT=<dir> ./script/crash-nodes.sh <index...>             hard-crash (SIGKILL), e.g. 2 7 12
#   DB_ROOT=<dir> ./script/crash-nodes.sh random <N>             hard-crash N random running nodes
#   DB_ROOT=<dir> ./script/crash-nodes.sh graceful <index...>    Ctrl-C (SIGINT): the node leaves the
#                                                                libp2p swarm / kademlia and shuts down
#                                                                its openraft groups cleanly
#   DB_ROOT=<dir> ./script/crash-nodes.sh graceful random <N>    graceful stop of N random nodes
#
# DB_ROOT defaults to the newest cluster under ${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}.
# CRASH_SIGNAL overrides the signal for the non-graceful mode (default: KILL).
# GRACEFUL_WAIT_SECS bounds how long the graceful mode waits per node (default: 40).
# Stopped indices are recorded in $DB_ROOT/crashed-nodes.txt for restart-nodes.sh.
set -euo pipefail

DB_BASE="${DB_BASE:-/tmp/openraft_libp2p_cluster_demo}"
DB_ROOT="${DB_ROOT:-}"
HTTP_PORT_BASE="${HTTP_PORT_BASE:-3000}"
CRASH_SIGNAL="${CRASH_SIGNAL:-KILL}"
GRACEFUL_WAIT_SECS="${GRACEFUL_WAIT_SECS:-40}"
GRACEFUL=0

usage() {
	echo "Usage: DB_ROOT=<dir> $0 [graceful] <index...> | [graceful] random <N>" >&2
	echo "Examples:" >&2
	echo "  DB_ROOT=/tmp/openraft_libp2p_cluster_demo/<run-id> $0 2 7           # SIGKILL crash" >&2
	echo "  DB_ROOT=/tmp/openraft_libp2p_cluster_demo/<run-id> $0 graceful 2 7  # Ctrl-C, clean exit" >&2
	exit 1
}

if [[ $# -lt 1 ]]; then
	usage
fi

if [[ "$1" == "graceful" ]]; then
	GRACEFUL=1
	CRASH_SIGNAL="INT"
	shift
	[[ $# -lt 1 ]] && usage
fi

if [[ -z "$DB_ROOT" ]]; then
	DB_ROOT="$(ls -td "$DB_BASE"/*/ 2>/dev/null | head -1 || true)"
	DB_ROOT="${DB_ROOT%/}"
	[[ -n "$DB_ROOT" ]] && echo "Using newest cluster: DB_ROOT=$DB_ROOT"
fi
if [[ -z "$DB_ROOT" || ! -d "$DB_ROOT" ]]; then
	echo "Error: no cluster data found. Start ./script/run-20nodes.sh first or export DB_ROOT." >&2
	exit 1
fi

CRASHED_FILE="$DB_ROOT/crashed-nodes.txt"
LOG_DIR="$DB_ROOT/logs"

# While waiting for graceful shutdowns, exit cleanly on the script's own
# Ctrl-C: the SIGINTs were already delivered, the nodes keep shutting down.
on_script_interrupt() {
	echo
	echo "Interrupted; the SIGINT(s) were already sent, nodes continue shutting down in the background." >&2
	exit 130
}
trap on_script_interrupt INT

# Find the pid of node <index> by matching its unique --db path in the
# process command line.
node_pid() {
	local index="$1"
	local db="$DB_ROOT/node${index}-${index}"
	local pid
	local cmdline
	while read -r pid; do
		cmdline="$(ps -p "$pid" -o command= 2>/dev/null || true)"
		if [[ "$cmdline" == *"--db $db "* || "$cmdline" == *"--db $db" ]]; then
			printf '%s' "$pid"
			return 0
		fi
	done < <(pgrep -f openraft_libp2p_cluster || true)
	return 1
}

# All node indices that have a data dir in this cluster.
all_indices() {
	local dir
	local base
	for dir in "$DB_ROOT"/node*-*/; do
		[[ -d "$dir" ]] || continue
		base="$(basename "$dir")"
		base="${base#node}"
		printf '%s\n' "${base%%-*}"
	done | sort -n
}

running_indices() {
	local index
	for index in $(all_indices); do
		if node_pid "$index" >/dev/null; then
			printf '%s\n' "$index"
		fi
	done
}

shuffle_lines() {
	# Portable shuffle (no shuf dependency): decorate with $RANDOM, sort, strip.
	while read -r line; do
		printf '%s %s\n' "$RANDOM" "$line"
	done | sort -n | cut -d' ' -f2-
}

TARGETS=()
if [[ "$1" == "random" ]]; then
	COUNT="${2:-}"
	if ! [[ "$COUNT" =~ ^[0-9]+$ ]] || ((COUNT < 1)); then
		usage
	fi
	while read -r index; do
		((${#TARGETS[@]} < COUNT)) && TARGETS+=("$index")
	done < <(running_indices | shuffle_lines)
	if ((${#TARGETS[@]} == 0)); then
		echo "Error: no running nodes found under $DB_ROOT." >&2
		exit 1
	fi
	echo "Randomly selected nodes: ${TARGETS[*]}"
else
	for arg in "$@"; do
		if ! [[ "$arg" =~ ^[0-9]+$ ]]; then
			usage
		fi
		TARGETS+=("$arg")
	done
fi

node_peer_id() {
	local index="$1"
	local peer_file="$DB_ROOT/node${index}-${index}/peer.id"
	if [[ -s "$peer_file" ]]; then
		cat "$peer_file"
	else
		printf 'unknown'
	fi
}

# Peer ids that are currently OpenRaft voters (leader/follower in any raft
# group). Control membership is decided at runtime by the join protocol —
# no fixed node index is a control node — so ask a running node for the
# actual membership instead of assuming indices.
openraft_voter_peer_ids() {
	local index
	local body
	for index in $(running_indices); do
		body="$(curl -fsS -m 3 "http://127.0.0.1:$((HTTP_PORT_BASE + index))/openraft/nodes" 2>/dev/null)" || continue
		printf '%s' "$body" |
			grep -o '"node_id":"[^"]*"[^{]*"role":"[^"]*"' |
			grep -E '"role":"(leader|follower)"' |
			sed 's/"node_id":"\([^"]*\)".*/\1/' |
			sort -u
		return 0
	done
	return 1
}

# Wait for a pid to exit, then confirm the node logged its clean shutdown.
# $3 = number of log lines that existed before the signal was sent, so we
# only search the freshly appended region (the log survives restarts).
wait_graceful_exit() {
	local index="$1"
	local pid="$2"
	local log_lines_before="$3"
	local log="$LOG_DIR/node${index}.log"
	local start=$SECONDS

	while kill -0 "$pid" 2>/dev/null; do
		if ((SECONDS - start >= GRACEFUL_WAIT_SECS)); then
			echo "node$index (pid=$pid) did not exit within ${GRACEFUL_WAIT_SECS}s after SIGINT." >&2
			return 1
		fi
		sleep 0.5
	done

	if [[ -f "$log" ]] && tail -n "+$((log_lines_before + 1))" "$log" | grep -q "shutdown complete"; then
		echo "node$index exited gracefully in $((SECONDS - start))s (left kademlia, stopped libp2p swarm and openraft groups; 'shutdown complete' confirmed)."
	else
		echo "node$index exited in $((SECONDS - start))s (no 'shutdown complete' line found in $log; check the log)." >&2
	fi
	return 0
}

STOPPED=()
STOPPED_NAMES=()
SKIPPED_NAMES=()
UNGRACEFUL_NAMES=()
VOTERS_STOPPED=0
declare -a GRACE_PIDS=()
declare -a GRACE_LOG_LINES=()

# Snapshot the real voter membership while the targets are still running so
# the quorum warning below reflects which nodes actually are voters.
VOTER_PEER_IDS="$(openraft_voter_peer_ids || true)"
VOTER_COUNT=0
if [[ -n "$VOTER_PEER_IDS" ]]; then
	VOTER_COUNT="$(printf '%s\n' "$VOTER_PEER_IDS" | grep -c .)"
fi

for index in "${TARGETS[@]}"; do
	if pid="$(node_pid "$index")"; then
		if ((GRACEFUL)); then
			log="$LOG_DIR/node${index}.log"
			lines_before=0
			[[ -f "$log" ]] && lines_before="$(wc -l <"$log" | tr -d ' ')"
			GRACE_PIDS+=("$pid")
			GRACE_LOG_LINES+=("$lines_before")
			echo "sent Ctrl-C (SIGINT) to node$index (pid=$pid, peer=$(node_peer_id "$index"))"
		else
			echo "crashed node$index (pid=$pid, signal=$CRASH_SIGNAL, peer=$(node_peer_id "$index"))"
		fi
		# Journal the deliberate stop so the run-20nodes.sh monitor reports
		# it as a crash drill instead of an unexplained node exit.
		echo "$(date '+%Y-%m-%dT%H:%M:%S') node=${index} pid=${pid} SIG${CRASH_SIGNAL} by crash-nodes.sh" \
			>>"$DB_ROOT/crash-drill.log"
		kill "-$CRASH_SIGNAL" "$pid"
		STOPPED+=("$index")
		STOPPED_NAMES+=("node$index")
		if [[ -n "$VOTER_PEER_IDS" ]] &&
			printf '%s\n' "$VOTER_PEER_IDS" | grep -qxF "$(node_peer_id "$index")"; then
			VOTERS_STOPPED=$((VOTERS_STOPPED + 1))
		fi
	else
		echo "node$index is not running; skipped." >&2
		SKIPPED_NAMES+=("node$index")
	fi
done

if ((${#STOPPED[@]} == 0)); then
	echo "No nodes were stopped." >&2
	echo "Crashed nodes: (none)"
	exit 1
fi

# In graceful mode, wait for every signaled node to finish its shutdown and
# verify the clean-exit marker in its log.
if ((GRACEFUL)); then
	echo "Waiting up to ${GRACEFUL_WAIT_SECS}s for graceful shutdowns..."
	for i in "${!STOPPED[@]}"; do
		if ! wait_graceful_exit "${STOPPED[$i]}" "${GRACE_PIDS[$i]}" "${GRACE_LOG_LINES[$i]}"; then
			UNGRACEFUL_NAMES+=("node${STOPPED[$i]}")
		fi
	done
fi

# Record for restart-nodes.sh (dedup + keep sorted).
{
	[[ -f "$CRASHED_FILE" ]] && cat "$CRASHED_FILE"
	printf '%s\n' "${STOPPED[@]}"
} | sort -n -u >"$CRASHED_FILE.tmp"
mv "$CRASHED_FILE.tmp" "$CRASHED_FILE"

echo
if ((GRACEFUL)); then
	echo "Gracefully stopped nodes: ${STOPPED_NAMES[*]}"
else
	echo "Crashed nodes: ${STOPPED_NAMES[*]}"
fi
if ((${#SKIPPED_NAMES[@]} > 0)); then
	echo "Skipped (not running): ${SKIPPED_NAMES[*]}"
fi
if ((${#UNGRACEFUL_NAMES[@]} > 0)); then
	echo "Still running after SIGINT (inspect or re-run without graceful): ${UNGRACEFUL_NAMES[*]}" >&2
fi
for index in "${STOPPED[@]}"; do
	echo "  node$index peer=$(node_peer_id "$index") http=http://127.0.0.1:$((${HTTP_PORT_BASE:-3000} + index))"
done
echo "Recorded stopped nodes in $CRASHED_FILE: $(paste -sd' ' "$CRASHED_FILE")"
if [[ -z "$VOTER_PEER_IDS" ]]; then
	echo "Note: could not query the openraft membership before stopping; unable to tell whether voters went down."
elif ((VOTERS_STOPPED > (VOTER_COUNT - 1) / 2)); then
	echo "WARNING: $VOTERS_STOPPED of $VOTER_COUNT raft voters are down; raft quorum is LOST until enough voters restart." >&2
elif ((VOTERS_STOPPED > 0)); then
	echo "Note: $VOTERS_STOPPED raft voter(s) stopped. The membership guard replaces a dead voter"
	echo "      with a learner after --voter-replace-timeout-secs (run-20nodes.sh default: 60s);"
	echo "      restart it sooner with ./script/restart-nodes.sh to keep its voter seat."
fi
echo "Note: dead learners are removed from the membership and dead workers are pruned from the"
echo "      known-nodes address book after the same timeout; until then they show connected=false."
echo "Restart with: DB_ROOT=$DB_ROOT ./script/restart-nodes.sh"
