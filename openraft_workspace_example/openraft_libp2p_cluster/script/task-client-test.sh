#!/usr/bin/env bash
# End-to-end accuracy test of the control + worker task architecture using
# the `olpc-task` client. Run it against a live cluster started by
# ./script/run-20nodes.sh.
#
# Phases:
#   1. preflight     control/worker HTTP reachable, active workers present
#   2. correctness   N tasks via control + M via a worker endpoint → all Done
#                    with exactly one attempt each, spread across workers,
#                    and both entry paths agree on the same task list
#   3. idempotency   same idem key pushed twice → single task, dedup reply
#   4. failure       fail* task → Failed after exactly MAX attempts (3)
#   5. metrics       /tasks/metrics agrees with the task list
#   6. multi-kind    digest / sleep / kv_set / webhook (task-chaining) pushed
#                    together → all Done; kinds run side by side, not
#                    mutually exclusive
#   7. crash drill   (WITH_CRASH=1) kill a worker mid-burst → still all Done,
#                    then restart it
#
# Usage:
#   ./script/task-client-test.sh                        # defaults below
#   CONTROL_HTTP=127.0.0.1:3001 WORKER_HTTP=127.0.0.1:3006 \
#     BATCH=6 WITH_CRASH=1 ./script/task-client-test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WS_DIR="$(cd "$ROOT_DIR/.." && pwd)"

CONTROL_HTTP="${CONTROL_HTTP:-127.0.0.1:3001}"
WORKER_HTTP="${WORKER_HTTP:-127.0.0.1:3006}"
BATCH="${BATCH:-6}"
WITH_CRASH="${WITH_CRASH:-0}"
CRASH_NODE="${CRASH_NODE:-6}"
SETTLE_TIMEOUT="${SETTLE_TIMEOUT:-120}"
FAIL_SETTLE_TIMEOUT="${FAIL_SETTLE_TIMEOUT:-150}"

# Unique per invocation. Task records (and idempotency keys) are durable in
# the raft state machine and survive across test runs, so every identifier a
# phase counts on must be run-scoped: a fixed idem key would collide with the
# previous run's record and the "first" push would silently deduplicate
# instead of creating a row, throwing every incremental --expect-total off by
# one from phase 3 onward.
RUN_ID="$(date +%s)-$$"

TASK_BIN="${CARGO_TARGET_DIR:-$WS_DIR/target}/debug/olpc-task"
if [[ ! -x "$TASK_BIN" ]]; then
	echo "Building olpc-task client..."
	(cd "$WS_DIR" && cargo build -p openraft_libp2p_cluster --bin olpc-task >/dev/null) || exit 1
fi

PASS=0
FAIL=0
check() {
	local name="$1"
	shift
	if "$@"; then
		echo "PASS: $name"
		PASS=$((PASS + 1))
	else
		echo "FAIL: $name" >&2
		FAIL=$((FAIL + 1))
	fi
}

task() {
	"$TASK_BIN" --http "$CONTROL_HTTP" "$@"
}

task_via_worker() {
	"$TASK_BIN" --http "$WORKER_HTTP" "$@"
}

echo "== phase 1: preflight =="
check "control HTTP reachable" curl -fsS -m 5 "http://$CONTROL_HTTP/cluster" -o /dev/null
check "worker HTTP reachable" curl -fsS -m 5 "http://$WORKER_HTTP/cluster" -o /dev/null

active_workers="$(task metrics 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["active_workers"])' 2>/dev/null || echo 0)"
echo "active workers: $active_workers"
check "at least one active worker" test "${active_workers:-0}" -ge 1

if ((FAIL > 0)); then
	echo "Preflight failed; is the cluster running? (./script/run-20nodes.sh)" >&2
	exit 1
fi

existing_total="$(task metrics | python3 -c 'import json,sys; print(json.load(sys.stdin)["total"])')"
existing_failed="$(task metrics | python3 -c 'import json,sys; print(json.load(sys.stdin)["failed"])')"
if ((existing_total > 0)); then
	echo "Note: cluster already holds $existing_total task record(s) ($existing_failed failed); expectations are computed incrementally."
fi

min_workers=1
if ((active_workers >= 2)); then
	min_workers=2
fi

echo
echo "== phase 2: correctness (${BATCH} via control + ${BATCH} via worker) =="
check "push via control node" task push --to ok-ctrl@example.com --count "$BATCH"
check "push via worker node (tarpc path)" task_via_worker push --to ok-worker@example.com --count "$BATCH"

total_after_p2=$((existing_total + 2 * BATCH))
check "all tasks settle Done with exactly one attempt, spread over >=${min_workers} workers" \
	task watch --timeout-secs "$SETTLE_TIMEOUT" \
	--expect-total "$total_after_p2" \
	--expect-failed "$existing_failed" \
	--expect-single-attempt-done \
	--min-distinct-workers "$min_workers"

echo
echo "== phase 3: idempotency =="
# Run-scoped recipient AND idem key: a fixed key would dedup against the
# previous run's durable record, so no new row would be created this run.
idem_to="idem-${RUN_ID}@example.com"
idem_key="same-key-${RUN_ID}"
first="$(task push --to "$idem_to" --idem "$idem_key" | grep -o 'task_id=[^ ]*')"
second="$(task push --to "$idem_to" --idem "$idem_key" | tail -1)"
echo "first:  $first"
echo "second: $second"
check "second push is deduplicated" grep -q "deduplicated=true" <<<"$second"
check "second push returns the original task id" grep -q "${first#task_id=}" <<<"$second"
total_after_p3=$((total_after_p2 + 1))
check "idempotent push created exactly one record" \
	task watch --timeout-secs "$SETTLE_TIMEOUT" --expect-total "$total_after_p3" --expect-failed "$existing_failed"

echo
echo "== phase 4: failure semantics (retry with backoff, then permanent) =="
# Run-scoped recipient (the "fail" prefix is what triggers the simulated
# failure): a fixed address would leave one failed row per past run, and the
# attempts check below would match all of them instead of just this run's.
fail_to="fail-drill-${RUN_ID}@example.com"
check "push failing task" task push --to "$fail_to"
total_after_p4=$((total_after_p3 + 1))
# backoff: 10s + 20s between attempts → give it time to exhaust 3 attempts
check "failing task ends Failed after retries; everything else stays Done" \
	task watch --timeout-secs "$FAIL_SETTLE_TIMEOUT" \
	--expect-total "$total_after_p4" \
	--expect-failed "$((existing_failed + 1))"
attempts="$(task list --status failed | awk -v to="$fail_to" '$0 ~ to {print $3}')"
echo "fail-drill attempts: $attempts"
check "failing task used exactly MAX_TASK_ATTEMPTS (3)" test "${attempts:-0}" = "3"

echo
echo "== phase 5: metrics consistency =="
metrics_json="$(task metrics)"
echo "$metrics_json"
list_total="$(task list | tail -n +2 | wc -l | tr -d ' ')"
check "metrics.total matches task list length" python3 - "$metrics_json" "$list_total" << 'EOF'
import json, sys
metrics = json.loads(sys.argv[1])
assert metrics["total"] == int(sys.argv[2]), (metrics["total"], sys.argv[2])
assert metrics["queued"] + metrics["assigned"] + metrics["running"] \
  + metrics["done"] + metrics["failed"] == metrics["total"]
EOF

echo
echo "== phase 6: multi-kind tasks (generic handler registry; kinds are not mutually exclusive) =="
# Five kinds pushed back to back so they overlap on the workers: CPU-bound
# digest, timed sleep, a raft KV write-back, a webhook that POSTs to the
# control node's own /tasks/email — chaining one extra email task — and a
# wasm task whose HANDLER travels inside the payload (+6 total).
check "push digest task" task push-task \
	--payload '{"kind":"digest","data":"octopii","iterations":100000}'
check "push sleep task (via worker endpoint)" task_via_worker push-task \
	--payload '{"kind":"sleep","secs":2}'
check "push kv_set task" task push-task \
	--payload '{"kind":"kv_set","key":"task:multi-kind","value":"written-by-task"}'
check "push webhook task (chains an email task)" task push-task \
	--payload '{"kind":"webhook","url":"http://'"$CONTROL_HTTP"'/tasks/email","body":{"to":"chained@example.com"}}'
# Code-as-data: the processing function itself is a WASI module (WAT text)
# carried in the payload; the worker runs it under wasmtime (p2 component
# runtime) and stores its stdout as the task result. Pushed with the
# dedicated `push-wasm` client command, which reads the module from a file.
check "push wasm task (handler stored as data, via push-wasm)" \
	task push-wasm --wat-file "$SCRIPT_DIR/wat/hello.wat" --name hello

# The richer stored handler: prime_count.wat PARSES its input from argv,
# COMPUTES (trial-division prime counting — a real fuel-burning loop) and
# FORMATS its own decimal output. π(1000) = 168, asserted exactly below.
check "push wasm prime-count task (argv in, computed stdout out)" \
	task push-wasm --wat-file "$SCRIPT_DIR/wat/prime_count.wat" \
	--name prime-count --arg 1000

# The richest stored handler: stats.wat folds a LIST of argv numbers into
# count/sum/min/max/avg, reads the LABEL env var, and assembles a JSON
# report INSIDE the guest — every input/output channel at once. The
# result's stdout is machine-checkable JSON, asserted field by field below.
check "push wasm stats task (argv list + env in, guest-built JSON out)" \
	task push-wasm --wat-file "$SCRIPT_DIR/wat/stats.wat" \
	--name stats --arg 12 --arg 7 --arg 42 --arg 19 --env LABEL=cluster-run

# Server-side guard: a payload past MAX_TASK_PAYLOAD_BYTES (256 KiB) must be
# rejected at the enqueue door before it can reach the raft log.
big_payload="$(python3 -c 'import json; print(json.dumps({"kind":"wasm","module_wat":"x"*300_000,"name":"oversized"}))')"
check "oversized wasm payload is rejected before the raft log" \
	bash -c '! '"$TASK_BIN"' --http '"$CONTROL_HTTP"' push-task --payload "$1" 2>/dev/null' _ "$big_payload"
total_after_p6=$((total_after_p4 + 8))
check "all kinds settle Done side by side" \
	task watch --timeout-secs "$SETTLE_TIMEOUT" \
	--expect-total "$total_after_p6" \
	--expect-failed "$((existing_failed + 1))"
done_has() {
	# done_has <label-pattern> [<content-pattern>]: the Done list has a row
	# matching both patterns.
	local rows
	rows="$(task list --status done | grep -- "$1")" || return 1
	grep -q -- "${2:-}" <<<"$rows"
}
check "digest task recorded a sha256 result" done_has 'digest:octopii' 'sha256'
# The list output truncates the RESULT column, so do not grep the ack string
# there; verify the write end to end instead by reading the key back from
# the users raft group's state machine.
check "kv_set task settled Done" done_has 'kv_set:task:multi-kind'
kv_value="$(curl -fsS -m 5 "http://$CONTROL_HTTP/cluster?group_id=users" | python3 -c '
import json, sys
data = json.load(sys.stdin)
pairs = data.get("kv_data") or []
print(next((p["value"] for p in pairs if p["key"] == "task:multi-kind"), ""))
')"
check "kv_set write landed in the users raft group" test "$kv_value" = "written-by-task"
check "webhook chained email completed" done_has 'email:chained@example.com'
check "wasm task settled Done" done_has 'wasm:hello'
check "wasm prime-count task settled Done" done_has 'wasm:prime-count'
# Verify the stored-handler outputs end to end from the full record JSON
# (the list view truncates the RESULT column): hello's fixed string, and
# prime-count's COMPUTED answer for argv 1000.
wasm_stdout() {
	curl -fsS -m 5 "http://$CONTROL_HTTP/tasks" | python3 -c '
import json, sys
name = sys.argv[1]
for t in json.load(sys.stdin)["tasks"]:
    if f"\"name\":\"{name}\"" in t.get("payload", "") and t.get("result"):
        print(json.loads(t["result"]).get("stdout", ""), end="")
        break
' "$1"
}
check "wasm handler stdout captured as task result" \
	grep -q "hello from wasm task" <<<"$(wasm_stdout hello)"
prime_out="$(wasm_stdout prime-count)"
echo "prime-count(1000) stdout: ${prime_out%$'\n'}"
check "wasm prime-count computed pi(1000) = 168" test "${prime_out%$'\n'}" = "168"
check "wasm stats task settled Done" done_has 'wasm:stats'
stats_out="$(wasm_stdout stats)"
echo "stats stdout: ${stats_out%$'\n'}"
check "wasm stats guest-built JSON has every field right" \
	python3 -c '
import json, sys
report = json.loads(sys.argv[1])
assert report == {"label": "cluster-run", "count": 4, "sum": 80,
                  "min": 7, "max": 42, "avg": 20}, report
' "$stats_out"

if [[ "$WITH_CRASH" == "1" ]]; then
	echo
	echo "== phase 7: crash drill (kill worker node${CRASH_NODE} mid-burst) =="
	task push --to crash-burst@example.com --count "$BATCH" >/dev/null &
	push_pid=$!
	"$SCRIPT_DIR/crash-nodes.sh" "$CRASH_NODE" >/dev/null 2>&1
	wait "$push_pid"
	total_after_p7=$((total_after_p6 + BATCH))
	check "burst completes despite worker crash" \
		task watch --timeout-secs "$SETTLE_TIMEOUT" \
		--expect-total "$total_after_p7" \
		--expect-failed "$((existing_failed + 1))"
	"$SCRIPT_DIR/restart-nodes.sh" "$CRASH_NODE" >/dev/null 2>&1
	check "crashed worker restarted" curl -fsS -m 10 "http://127.0.0.1:$((3000 + CRASH_NODE))/cluster" -o /dev/null
fi

echo
echo "================================"
echo "task architecture test: PASS=$PASS FAIL=$FAIL"
((FAIL == 0)) && echo "RESULT: PASS" || echo "RESULT: FAIL"
exit "$((FAIL > 0 ? 1 : 0))"
