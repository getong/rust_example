const value = document.getElementById('value');
const revision = document.getElementById('revision');
const status = document.getElementById('status');
const buttons = [...document.querySelectorAll('button')];
let busy = false;
async function sync(action) {
  if (busy) return;
  busy = true;
  buttons.forEach(button => button.disabled = true);
  try {
    const response = await fetch('/api/counter', {
      method: action ? 'POST' : 'GET',
      headers: action ? { 'Content-Type': 'application/json' } : {},
      body: action ? JSON.stringify({ action }) : undefined,
      cache: 'no-store',
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const snapshot = await response.json();
    value.textContent = snapshot.value;
    revision.textContent = snapshot.revision;
    status.textContent = '已连接 · 每两秒同步';
  } catch (error) {
    status.textContent = `连接失败，显示的数值可能已过期：${error.message}。修改操作不会自动重试，请先刷新确认。`;
  } finally {
    busy = false;
    buttons.forEach(button => button.disabled = false);
  }
}
document.getElementById('increment').onclick = () => sync('increment');
document.getElementById('reset').onclick = () => sync('reset');
document.getElementById('refresh').onclick = () => sync();
sync();
setInterval(() => sync(), 2000);
