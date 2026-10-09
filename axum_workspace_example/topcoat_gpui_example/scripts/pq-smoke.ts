// Cross-implementation test: browser TypeScript (noble/WebCrypto) ↔ Rust (AWS-LC).
import assert from 'node:assert/strict';
import { encryptedRequest } from '../frontend/secure.ts';
const base = process.env.TOPCOAT_URL ?? 'http://127.0.0.1:3000';
const original = globalThis.fetch;
const seen: string[] = [];
globalThis.fetch = (input, options) => {
  const path = String(input); seen.push(path);
  assert.ok(path === '/pq/handshake' || path === '/pq/exchange');
  if (path === '/pq/exchange') {
    assert.equal(typeof options?.body, 'string');
    assert.ok(!String(options?.body).includes('browser secret'));
    assert.ok(!String(options?.body).includes('/api/'));
  }
  return original(new URL(path, base), options);
};
async function call(path: string, body?: unknown): Promise<Record<string, unknown>> {
  return await encryptedRequest({method: body === undefined ? 'GET' : 'POST', path, body: body === undefined ? '' : JSON.stringify(body), form: false}) as Record<string, unknown>;
}
try {
  if (process.argv.includes('--expect-native')) {
    const state = await call('/api/demos');
    assert.ok(JSON.stringify(state).includes('GPUI task'));
    assert.ok(JSON.stringify(state).includes('GPUI User'));
  }
  const before = await call('/api/counter');
  const after = await call('/api/counter', {action: 'increment'});
  assert.equal(after.value, Number(before.value) + 1);
  const todos = await call('/api/todos', {action: 'create', title: 'Browser task'});
  assert.ok(JSON.stringify(todos).includes('Browser task'));
  assert.ok(JSON.stringify(await call('/api/demos')).includes('Browser task'));
  assert.ok(JSON.stringify(await call('/api/echo', {source: 'browser', message: 'browser secret'})).includes('browser secret'));
  assert.ok(JSON.stringify(await call('/api/profile', {username: 'Browser User', age: 25})).includes('Browser User'));
  assert.equal((await call('/api/studio', {action: 'apply', theme: 'forest', intensity: 41})).intensity, 41);
  await assert.rejects(call('/api/profile', {username: '', age: 20}), /HTTP 400/);
  const prior = seen.length;
  const normalFetch = globalThis.fetch;
  globalThis.fetch = async (input, options) => {
    const response = await normalFetch(input, options);
    if (String(input) !== '/pq/handshake') return response;
    const hello = await response.json() as Record<string, string>;
    hello.public_key = '00'.repeat(2592);
    return new Response(JSON.stringify(hello));
  };
  await assert.rejects(call('/api/counter'));
  globalThis.fetch = normalFetch;
  assert.equal(seen.length, prior + 1); // No business request after a tampered hello.
  assert.equal((await original(`${base}/api/counter`)).status, 403);
  console.log('PASS: browser/AWS-LC interoperability, all APIs, encrypted errors, tampered dynamic key, plaintext rejection');
} finally { globalThis.fetch = original; }
