import { test } from 'node:test';
import assert from 'node:assert/strict';
import { counterSnapshot, demoSnapshot } from '../frontend/api.ts';
import { decodeStudio, studioApi } from '../frontend/studio-api.ts';
test('validates palette protocol, integer bounds and existing responses', () => {
  assert.deepEqual(decodeStudio({theme: 'ocean', intensity: 0, revision: 1}), {theme: 'ocean', intensity: 0, revision: 1});
  for (const data of [null, {}, {theme: 'bad', intensity: 50, revision: 0},
    {theme: 'ocean', intensity: 101, revision: 0}, {theme: 'ocean', intensity: 1.5, revision: 0},
    {theme: 'ocean', intensity: 50, revision: Number.MAX_SAFE_INTEGER + 1}]) assert.throws(() => decodeStudio(data));
  assert.throws(() => counterSnapshot({value: Number.MAX_SAFE_INTEGER + 1, revision: 0}));
  assert.throws(() => demoSnapshot({todos: [{id: 1, title: 'task', done: 'false'}], echoes: [], profiles: []}));
});
test('studio uses only its own endpoint and does not retry a failed publish', async () => {
  const original = globalThis.fetch;
  const calls: {path: string; body: string | undefined}[] = [];
  try {
    globalThis.fetch = async (input, options) => {
      calls.push({path: String(input), body: typeof options?.body === 'string' ? options.body : undefined});
      return new Response(JSON.stringify({theme: 'sunset', intensity: 35, revision: 1}));
    };
    await studioApi.read(); await studioApi.apply('sunset', 35);
    assert.deepEqual(calls.map(call => call.path), ['/api/studio', '/api/studio']);
    assert.deepEqual(JSON.parse(calls[1]!.body!), {action: 'apply', theme: 'sunset', intensity: 35});
    let failed = 0;
    globalThis.fetch = async () => { failed++; throw new Error('Disconnected'); };
    await assert.rejects(studioApi.apply('forest', 80), /Disconnected/);
    assert.equal(failed, 1);
  } finally { globalThis.fetch = original; }
});
