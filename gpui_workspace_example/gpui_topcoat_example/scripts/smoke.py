#!/usr/bin/env python3
"""Isolated PQ browser/Rust interoperability; never sends plaintext business data."""
from pathlib import Path
import os
import socket
import subprocess
import time
import urllib.request
import urllib.error

workspace = Path(__file__).resolve().parents[2]
frontend = Path(os.environ.get('TOPCOAT_TEST_FRONTEND', workspace.parent / 'axum_workspace_example/topcoat_gpui_example'))
backend = Path(os.environ.get('TOPCOAT_TEST_BACKEND', workspace.parent / 'axum_workspace_example/target/debug/topcoat_gpui_example'))
client = workspace / 'target/debug/topcoat-smoke'
assert backend.is_file() and client.is_file(), 'Build the server and topcoat-smoke first (see README).'
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]
url = f'http://127.0.0.1:{port}'
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
env = dict(os.environ, HOST='127.0.0.1', PORT=str(port), TOPCOAT_URL=url)
with subprocess.Popen([str(backend)], env=env) as server:
    try:
        for _ in range(100):
            assert server.poll() is None, 'Server exited during startup'
            try:
                with http.open(url + '/', timeout=0.5) as response:
                    assert response.status == 200
                break
            except OSError:
                time.sleep(0.05)
        else:
            raise RuntimeError('Server did not become ready')
        for path in ['/', '/todos', '/echo', '/profile', '/studio', '/app.js', '/demos.js', '/assets/studio']:
            with http.open(url + path, timeout=5) as response:
                assert response.status == 200
        try:
            http.open(url + '/api/counter', timeout=5)
            raise AssertionError('Plaintext API must be forbidden')
        except urllib.error.HTTPError as error:
            assert error.code == 403
        subprocess.run([str(client)], env=env, check=True, timeout=30)
        subprocess.run(['bun', 'scripts/pq-smoke.ts', '--expect-native'], cwd=frontend, env=env, check=True, timeout=30)
        subprocess.run([str(client), '--expect-browser-seed'], env=env, check=True, timeout=30)
        print('PASS: native → encrypted browser reads/writes → native, all pages and APIs')
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
failed = subprocess.run([str(client)], env=env, capture_output=True, timeout=15)
assert failed.returncode != 0, 'Disconnected server must return an error'
print('PASS: disconnected server produces a client error')
