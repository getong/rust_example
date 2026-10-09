import { encryptedRequest } from './secure';
import type { CounterAction, CounterSnapshot, DemoSnapshot, JsonValue, Profile, TodoCommand } from './types';

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function unsigned(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
function jsonValue(value: unknown): value is JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(jsonValue);
  return record(value) && Object.values(value).every(jsonValue);
}
export function counterSnapshot(value: unknown): CounterSnapshot {
  if (!record(value) || !unsigned(value.value) || !unsigned(value.revision)) {
    throw new Error('计数器响应无效，或整数超出浏览器安全范围');
  }
  return { value: value.value, revision: value.revision };
}
export function demoSnapshot(value: unknown): DemoSnapshot {
  if (!record(value) || !Array.isArray(value.todos) || !Array.isArray(value.echoes) || !Array.isArray(value.profiles)) {
    throw new Error('共享数据响应格式无效');
  }
  const todos = value.todos.map((todo: unknown) => {
    if (!record(todo) || !unsigned(todo.id) || typeof todo.title !== 'string' || typeof todo.done !== 'boolean') {
      throw new Error('待办响应无效，或 ID 超出浏览器安全范围');
    }
    return { id: todo.id, title: todo.title, done: todo.done };
  });
  const profiles = value.profiles.map((profile: unknown) => {
    if (!record(profile) || typeof profile.username !== 'string' || !unsigned(profile.age) || profile.age > 150) {
      throw new Error('表单响应无效');
    }
    return { username: profile.username, age: profile.age };
  });
  if (!value.echoes.every(jsonValue)) throw new Error('JSON 回显响应无效');
  return { todos, profiles, echoes: value.echoes };
}
export async function request<T>(path: string, decode: (data: unknown) => T, body?: string, form = false): Promise<T> {
  const data: unknown = await encryptedRequest({ method: body === undefined ? 'GET' : 'POST', path, body: body ?? '', form });
  return decode(data);
}
export const api = {
  counter: (action?: CounterAction) => request('/api/counter', counterSnapshot, action === undefined ? undefined : JSON.stringify({ action })),
  demos: () => request('/api/demos', demoSnapshot),
  todo: (command: TodoCommand) => request('/api/todos', demoSnapshot, JSON.stringify(command)),
  echo: (value: JsonValue) => request('/api/echo', demoSnapshot, JSON.stringify(value)),
  profile: (profile: Profile) => request('/api/profile', demoSnapshot, JSON.stringify(profile)),
};
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
export function element<T extends HTMLElement>(id: string, kind: { new (...args: never[]): T }): T {
  const node = document.getElementById(id);
  if (!(node instanceof kind)) throw new Error(`页面缺少有效元素：${id}`);
  return node;
}
