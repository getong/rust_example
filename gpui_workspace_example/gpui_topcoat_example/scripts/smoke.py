#!/usr/bin/env python3
"""Run against an isolated local server; uses prebuilt binaries, always stops it."""
from pathlib import Path
import json
import os
import socket
import subprocess
import time
import urllib.request
import urllib.parse

workspace = Path(__file__).resolve().parents[2]
backend = workspace.parent / 'axum_workspace_example/target/debug/topcoat_gpui_example'
client = workspace / 'target/debug/topcoat-smoke'
assert backend.is_file() and client.is_file(), 'Build the server and topcoat-smoke first (see README).'
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]
url = f'http://127.0.0.1:{port}'
env = dict(os.environ, HOST='127.0.0.1', PORT=str(port), TOPCOAT_URL=url)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with subprocess.Popen([str(backend)], env=env) as server:
    try:
        for _ in range(100):
            assert server.poll() is None, 'Server exited during startup'
            try:
                with http.open(url + '/api/counter', timeout=0.5) as response:
                    assert json.load(response) == {'value': 0, 'revision': 0}
                break
            except OSError:
                time.sleep(0.05)
        else:
            raise RuntimeError('Server did not become ready')
        with http.open(url, timeout=5) as response:
            html = response.read().decode()
            assert 'Topcoat × GPUI-kit' in html and 'id="value"' in html
        with http.open(url + '/app.js', timeout=5) as response:
            assert 'javascript' in response.headers['Content-Type']
            assert b"fetch('/api/counter'" in response.read()
        for path in ['/todos', '/echo', '/profile']:
            with http.open(url + path, timeout=5) as response:
                assert 'id="editor"' in response.read().decode()
        with http.open(url + '/demos.js', timeout=5) as response:
            assert 'javascript' in response.headers['Content-Type']
        def post(path, body, form=False):
            data = (urllib.parse.urlencode(body) if form else json.dumps(body)).encode()
            request = urllib.request.Request(url + path, data=data, headers={
                'Content-Type': 'application/x-www-form-urlencoded' if form else 'application/json',
                'Origin': url,
            })
            with http.open(request, timeout=5) as response:
                return json.load(response)
        post('/api/todos', {'action': 'create', 'title': 'Browser task'})
        post('/api/echo', {'source': 'browser'})
        post('/api/profile', {'username': 'Browser User', 'age': 25}, True)
        subprocess.run([str(client), '--expect-browser-seed'], env=env, check=True, timeout=30)
        with http.open(url + '/api/demos', timeout=5) as response:
            state = json.load(response)
            assert any(todo['title'] == 'GPUI task' and todo['done'] for todo in state['todos'])
            assert state['echoes'][0]['source'] == 'gpui'
            assert state['profiles'][0]['username'] == 'GPUI User'
        print('PASS: browser requests → desktop client → browser reads, all four pages')
        with http.open(url + '/api/counter', timeout=5) as response:
            assert json.load(response) == {'value': 0, 'revision': 2}
        print('PASS: HTML, JavaScript, native HTTP client, shared server state')
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
failed = subprocess.run([str(client)], env=env, capture_output=True, timeout=8)
assert failed.returncode != 0, 'Disconnected server must return an error'
print('PASS: disconnected server produces a client error')
