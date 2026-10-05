const page = document.body.dataset.page;
const results = document.getElementById('results');
const status = document.getElementById('status');
let busy = false;
function render(snapshot) {
  results.replaceChildren();
  if (page === 'todos') {
    for (const todo of snapshot.todos) {
      const row = document.createElement('p');
      const label = document.createElement('span');
      label.textContent = `${todo.done ? '✓' : '○'} ${todo.title} `;
      const toggle = document.createElement('button');
      toggle.textContent = todo.done ? '恢复待办' : '完成';
      toggle.onclick = () => sync('/api/todos', { action: 'set_done', id: todo.id, done: !todo.done });
      const remove = document.createElement('button');
      remove.textContent = '删除';
      remove.onclick = () => sync('/api/todos', { action: 'delete', id: todo.id });
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
  if (!results.children.length) results.textContent = '暂无数据，请在任一端提交。';
}
async function sync(path = '/api/demos', body, form = false) {
  if (busy) return;
  busy = true;
  document.querySelectorAll('button').forEach(b => b.disabled = true);
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      headers: body === undefined ? {} : { 'Content-Type': form ? 'application/x-www-form-urlencoded' : 'application/json' },
      body: body === undefined ? undefined : form ? new URLSearchParams(body) : JSON.stringify(body),
      cache: 'no-store', signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}: ${await response.text()}`);
    render(await response.json());
    status.textContent = '已同步 · 修改操作不会自动重试';
  } catch (error) {
    status.textContent = `请求失败，当前数据可能已过期：${error.message}`;
  } finally {
    busy = false;
    document.querySelectorAll('button').forEach(b => b.disabled = false);
  }
}
document.getElementById('editor').onsubmit = event => {
  event.preventDefault();
  if (page === 'todos') sync('/api/todos', { action: 'create', title: document.getElementById('title').value });
  if (page === 'echo') {
    try { sync('/api/echo', JSON.parse(document.getElementById('json').value)); }
    catch (error) { status.textContent = `JSON 格式错误：${error.message}`; }
  }
  if (page === 'profile') sync('/api/profile', { username: document.getElementById('username').value, age: document.getElementById('age').value }, true);
};
document.getElementById('refresh').onclick = () => sync();
sync();
setInterval(() => sync(), 2000);
