import { test } from 'node:test';
import assert from 'node:assert/strict';
import { encryptedRequest, unhex } from '../frontend/secure.ts';
test('rejects malformed hex and never retries a failed encrypted operation', async () => {
  for (const value of ['0', 'gg', 'AA', '0000', null]) assert.throws(() => unhex(value, 1, true));
  const original = globalThis.fetch;
  let calls = 0;
  try {
    globalThis.fetch = async (input) => { calls++; assert.equal(String(input), '/pq/handshake'); throw new Error('Disconnected'); };
    await assert.rejects(encryptedRequest({method: 'POST', path: '/api/studio', body: '{"action":"apply","theme":"ocean","intensity":70}', form: false}), /Disconnected/);
    assert.equal(calls, 1);
  } finally { globalThis.fetch = original; }
});
