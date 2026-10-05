import { api, element, errorMessage } from './api';
import type { DemoSnapshot, JsonValue } from './types';
const page = document.body.dataset.page;
if (page !== 'todos' && page !== 'echo' && page !== 'profile') throw new Error('未知页面');
const results = element('results', HTMLElement);
const status = element('status', HTMLElement);
let busy = false;
function render(snapshot: DemoSnapshot): void {
  results.replaceChildren();
  if (page === 'todos') {
    for (const todo of snapshot.todos) {
      const row = document.createElement('p');
      row.dataset.todoId = String(todo.id);
      const label = document.createElement('span');
      label.textContent = `${todo.done ? '✓' : '○'} ${todo.title} `;
      const toggle = document.createElement('button');
      toggle.textContent = todo.done ? '恢复待办' : '完成';
      toggle.onclick = () => void sync(() => api.todo({ action: 'set_done', id: todo.id, done: !todo.done }));
      const remove = document.createElement('button');
      remove.textContent = '删除';
      remove.onclick = () => void sync(() => api.todo({ action: 'delete', id: todo.id }));
      row.append(label, toggle, remove);
      results.append(row);
    }
  } else {
    const items = page === 'echo' ? snapshot.echoes : snapshot.profiles;
    for (const item of items) {
      const row = document.createElement('pre');
      row.style.whiteSpace = 'pre-wrap';
      row.textContent = JSON.stringify(item, null, 2);
      results.append(row);
    }
  }
  if (!results.children.length) results.textContent = page === 'todos' ? '暂无待办，请在网页或桌面添加。' : '暂无数据，请在任一端提交。';
  setDisabled(busy);
}
function setDisabled(disabled: boolean): void {
  document.querySelectorAll('button').forEach(button => button.disabled = disabled);
}
async function sync(load: () => Promise<DemoSnapshot> = api.demos): Promise<void> {
  if (busy) return;
  busy = true;
  setDisabled(true);
  try {
    render(await load());
    status.textContent = '已同步 · 每两秒刷新 · 修改操作不会自动重试';
  } catch (error) {
    status.textContent = `请求失败，当前数据可能已过期：${errorMessage(error)}`;
  } finally {
    busy = false;
    setDisabled(false);
  }
}
element('editor', HTMLFormElement).onsubmit = event => {
  event.preventDefault();
  try {
    if (page === 'todos') {
      const title = element('title', HTMLInputElement).value.trim();
      if (!title || [...title].length > 120) throw new Error('待办标题须为 1–120 个字符');
      void sync(() => api.todo({ action: 'create', title }));
    } else if (page === 'echo') {
      const value = JSON.parse(element('json', HTMLTextAreaElement).value) as JsonValue;
      void sync(() => api.echo(value));
    } else {
      const username = element('username', HTMLInputElement).value.trim();
      const age = element('age', HTMLInputElement).valueAsNumber;
      if (!username || [...username].length > 80 || !Number.isInteger(age) || age < 0 || age > 150) {
        throw new Error('姓名须为 1–80 个字符，年龄须为 0–150 的整数');
      }
      void sync(() => api.profile({ username, age }));
    }
  } catch (error) { status.textContent = `输入无效：${errorMessage(error)}`; }
};
element('refresh', HTMLButtonElement).onclick = () => void sync();
void sync();
setInterval(() => void sync(), 2000);
