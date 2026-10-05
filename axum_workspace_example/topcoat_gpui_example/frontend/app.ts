import { api, element, errorMessage } from './api';
import type { CounterAction } from './types';
const value = element('value', HTMLElement);
const revision = element('revision', HTMLElement);
const status = element('status', HTMLElement);
const buttons = [...document.querySelectorAll('button')];
let busy = false;
async function sync(action?: CounterAction): Promise<void> {
  if (busy) return;
  busy = true;
  buttons.forEach(button => button.disabled = true);
  try {
    const snapshot = await api.counter(action);
    value.textContent = String(snapshot.value);
    revision.textContent = String(snapshot.revision);
    status.textContent = '已连接 · 每两秒同步';
  } catch (error) {
    status.textContent = `连接失败，显示的数值可能已过期：${errorMessage(error)}。修改操作不会自动重试，请先刷新确认。`;
  } finally {
    busy = false;
    buttons.forEach(button => button.disabled = false);
  }
}
element('increment', HTMLButtonElement).onclick = () => void sync('increment');
element('reset', HTMLButtonElement).onclick = () => void sync('reset');
element('refresh', HTMLButtonElement).onclick = () => void sync();
void sync();
setInterval(() => void sync(), 2000);
