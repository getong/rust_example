import { test } from 'node:test';
import assert from 'node:assert/strict';
import { counterSnapshot, demoSnapshot } from '../frontend/api.ts';
import { decodeStudio } from '../frontend/studio-api.ts';
test('validates palette protocol, integer bounds and existing responses', () => {
  assert.deepEqual(decodeStudio({theme: 'ocean', intensity: 0, revision: 1}), {theme: 'ocean', intensity: 0, revision: 1});
  for (const data of [null, {}, {theme: 'bad', intensity: 50, revision: 0},
    {theme: 'ocean', intensity: 101, revision: 0}, {theme: 'ocean', intensity: 1.5, revision: 0},
    {theme: 'ocean', intensity: 50, revision: Number.MAX_SAFE_INTEGER + 1}]) assert.throws(() => decodeStudio(data));
  assert.throws(() => counterSnapshot({value: Number.MAX_SAFE_INTEGER + 1, revision: 0}));
  assert.throws(() => demoSnapshot({todos: [{id: 1, title: 'task', done: 'false'}], echoes: [], profiles: []}));
});
