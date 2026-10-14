#!/usr/bin/env python3
"""Three real processes: replication, leader loss, catch-up, and loss of quorum.
Run: cargo build --locked && python3 scripts/smoke.py
Tests RocksDB recovery after SIGKILL, including a full cluster restart.
"""
import json
import os
from pathlib import Path
import queue
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / "target/debug/raft_example"


def main():
    if not hasattr(signal, "SIGSTOP"):
        raise SystemExit("This smoke test requires POSIX SIGSTOP/SIGCONT")
    with tempfile.TemporaryDirectory(prefix="raft-libp2p-") as directory:
        directory = Path(directory)
        sockets = [socket.socket() for _ in range(3)]
        try:
            for sock in sockets:
                sock.bind(("127.0.0.1", 0))
            addresses = [f"/ip4/127.0.0.1/tcp/{sock.getsockname()[1]}" for sock in sockets]
        finally:
            for sock in sockets:
                sock.close()
        subprocess.run([str(BINARY), "init", str(directory), *addresses], check=True, capture_output=True)
        events = queue.Queue()
        processes, logs, states, history = {}, {}, {}, []
        readers = {}
        paused = set()

        def reader(node_id, stream):
            for line in stream:
                events.put((node_id, json.loads(line)))

        def start(node_id):
            if node_id not in logs:
                logs[node_id] = (directory / f"node{node_id}.log").open("w+")
            process = subprocess.Popen([str(BINARY), "node", str(directory / f"node{node_id}.json")],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=logs[node_id], text=True, bufsize=1)
            processes[node_id] = process
            readers[node_id] = threading.Thread(target=reader, args=(node_id, process.stdout), daemon=True)
            readers[node_id].start()

        def crash(node_id):
            process = processes[node_id]
            process.kill()
            process.wait(timeout=5)
            readers[node_id].join(timeout=5)
            process.stdin.close()
            process.stdout.close()
            paused.add(node_id)

        def restart(node_id):
            # Old reader has exited; consume its remaining events before removing stale state.
            while not events.empty():
                collect(0)
            states.pop(node_id, None)
            start(node_id)
            paused.remove(node_id)

        def send(node_id, command):
            processes[node_id].stdin.write(command + "\n")
            processes[node_id].stdin.flush()

        def collect(timeout=0.1):
            try:
                node_id, event = events.get(timeout=timeout)
            except queue.Empty:
                return
            history.append((node_id, event))
            if event["event"] == "status":
                states[node_id] = event

        def wait(predicate, description, timeout=15):
            deadline = time.monotonic() + timeout
            poll_at = 0
            while time.monotonic() < deadline:
                for node_id, process in processes.items():
                    if node_id not in paused and process.poll() is not None:
                        raise AssertionError(f"node {node_id} exited: {process.returncode}")
                if time.monotonic() >= poll_at:
                    for node_id in processes.keys() - paused:
                        send(node_id, "status")
                    poll_at = time.monotonic() + 0.25
                collect()
                if predicate():
                    print("PASS:", description)
                    return
            raise AssertionError(f"timeout: {description}; states={states}")

        def leader():
            active = processes.keys() - paused
            leaders = [i for i in active if states.get(i, {}).get("role") == "Leader"]
            if len(leaders) == 1 and all(states.get(i, {}).get("leader") == leaders[0] for i in active):
                return leaders[0]
            return None

        def replicated(value):
            return all(states.get(i, {}).get("values", {}).get("name") == value for i in processes.keys() - paused)

        try:
            start(1)
            start(2)
            wait(lambda: leader() is not None, "two nodes elect a leader while node 3 is offline")
            send(leader(), "set name before-node3-starts")
            wait(lambda: replicated("before-node3-starts"), "majority commits while node 3 is offline")
            dial_logs = "".join((directory / f"node{i}.log").read_text() for i in (1, 2))
            assert "dial node 3" in dial_logs and "retry in" in dial_logs, dial_logs
            assert "Transport(" in dial_logs, dial_logs
            print("PASS: offline peer logs address, transport error and retry delay")
            start(3)
            wait(lambda: replicated("before-node3-starts") and leader() is not None,
                 "late-starting node 3 automatically connects and catches up")
            old = leader()
            follower = next(i for i in processes if i != old)
            send(follower, "set name rejected-follower-write")
            wait(lambda: any(i == follower and e["event"] == "error" and "not leader" in e["message"]
                            for i, e in history), "follower rejects writes and reports leader ID")
            send(old, "set name before")
            wait(lambda: replicated("before"), "protobuf messages replicate to all three processes")

            crash(old)
            wait(lambda: leader() is not None, "remaining majority elects a new leader")
            new = leader()
            assert new != old
            send(new, "set name after-failover")
            wait(lambda: replicated("after-failover"), "new leader commits with two processes")
            restart(old)
            wait(lambda: replicated("after-failover") and leader() is not None,
                 "killed leader reopens RocksDB and catches up over libp2p")

            current = leader()
            for node_id in processes:
                if node_id != current:
                    os.kill(processes[node_id].pid, signal.SIGSTOP)
                    paused.add(node_id)
            send(current, "set minority must-not-commit")
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline:
                collect()
            assert not any(e["event"] == "committed" and e.get("key") == "minority" for _, e in history)
            print("PASS: one remaining process cannot commit without quorum")
            for node_id in list(paused):
                os.kill(processes[node_id].pid, signal.SIGCONT)
                paused.remove(node_id)
            wait(lambda: leader() is not None, "cluster elects a leader after quorum is restored")
            send(leader(), "set name recovered")
            wait(lambda: replicated("recovered"), "cluster continues committing after recovery")
            # Kill every process. Recovery cannot depend on any surviving memory replica.
            for node_id in processes:
                crash(node_id)
            for node_id in processes:
                restart(node_id)
            wait(lambda: replicated("recovered") and leader() is not None,
                 "all three SIGKILLed processes restore committed data from RocksDB")
            send(leader(), "set name after-full-restart")
            wait(lambda: replicated("after-full-restart"), "new writes commit after full cluster restart")
            # EOF must disable the stdin select branch, not stop or spin the node.
            follower = next(i for i in processes if i != leader())
            processes[follower].stdin.close()
            time.sleep(0.3)
            assert processes[follower].poll() is None
            for process in processes.values():
                process.send_signal(signal.SIGINT)
            for process in processes.values():
                assert process.wait(timeout=5) == 0
            print("PASS: stdin EOF keeps node alive; Ctrl-C exits even with stdin open")
        except Exception:
            for node_id, log in logs.items():
                log.flush()
                log.seek(0)
                print(f"--- node {node_id} stderr ---\n{log.read()[-6000:]}", file=sys.stderr)
            raise
        finally:
            for node_id, process in processes.items():
                if process.poll() is None:
                    if node_id in paused:
                        os.kill(process.pid, signal.SIGCONT)
                    process.kill()
                    process.wait(timeout=5)
                if process.stdin and not process.stdin.closed:
                    process.stdin.close()
                process.stdout.close()
            for log in logs.values():
                log.close()


if __name__ == "__main__":
    main()
