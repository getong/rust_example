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

import { decodeSuggestions } from '../frontend/autocomplete';
test('autocomplete validates bounded response and renders user text as data', () => {
  assert.deepEqual(decodeSuggestions({query: '学', items: [{value: '学习 Rust', detail: 'shared', source: 'shared'}]}).items[0]?.value, '学习 Rust');
  for (const value of [null, {}, {query:'x', items:[{}]}, {query:'x',items:Array(7).fill({value:'x',detail:'',source:'shared'})}, {query:'x',items:[{value:'x',detail:'',source:'unknown'}]}]) assert.throws(() => decodeSuggestions(value));
});
