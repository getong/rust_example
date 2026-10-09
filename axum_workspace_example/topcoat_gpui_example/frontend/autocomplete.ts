import { encryptedRequest } from './secure';
export interface Suggestion { value: string; detail: string; source: 'shared' | 'suggested' }
export interface Suggestions { query: string; items: Suggestion[] }
export function decodeSuggestions(value: unknown): Suggestions {
  if (!value || typeof value !== 'object') throw new Error('补全响应无效');
  const data = value as Record<string, unknown>;
  if (typeof data.query !== 'string' || !Array.isArray(data.items) || data.items.length > 6) throw new Error('补全响应无效');
  const items = data.items.map((item: unknown): Suggestion => {
    if (!item || typeof item !== 'object') throw new Error('补全条目无效');
    const row = item as Record<string, unknown>;
    if (typeof row.value !== 'string' || !row.value || row.value.length > 240 || typeof row.detail !== 'string' || (row.source !== 'shared' && row.source !== 'suggested')) throw new Error('补全条目无效');
    return {value: row.value, detail: row.detail, source: row.source};
  });
  return {query: data.query, items};
}
export function attachAutocomplete(input: HTMLInputElement, kind: 'todo' | 'profile'): void {
  const wrapper = document.createElement('span'); wrapper.className = 'autocomplete';
  input.before(wrapper); wrapper.append(input);
  const panel = document.createElement('div'); panel.className = 'autocomplete-panel'; panel.hidden = true;
  const list = document.createElement('div'); list.id = `${input.id}-suggestions`; list.setAttribute('role', 'listbox'); list.setAttribute('aria-label', '补全建议');
  const status = document.createElement('div'); status.id = `${input.id}-suggestion-status`; status.className = 'autocomplete-status'; status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  panel.append(list, status); wrapper.append(panel);
  input.autocomplete = 'off'; input.setAttribute('role', 'combobox'); input.setAttribute('aria-autocomplete', 'list'); input.setAttribute('aria-expanded', 'false'); input.setAttribute('aria-controls', list.id); input.setAttribute('aria-describedby', status.id);
  input.placeholder = kind === 'todo' ? '输入「学」或 rust，发现建议…' : '输入「张」或 zxm，查找联系人…';
  let items: Suggestion[] = [], active = -1, generation = 0, composing = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: AbortController | undefined;
  function cancel(): void { generation++; clearTimeout(timer); pending?.abort(); pending = undefined; }
  function close(): void {
    cancel(); panel.hidden = true; items = []; active = -1; list.replaceChildren(); status.textContent = '';
    input.setAttribute('aria-expanded', 'false'); input.removeAttribute('aria-activedescendant');
  }
  function show(message: string): void { panel.hidden = false; input.setAttribute('aria-expanded', 'true'); status.textContent = message; }
  function select(index: number): void {
    active = index;
    Array.from(list.children).forEach((node, i) => node.setAttribute('aria-selected', String(i === active)));
    const option = list.children[active];
    if (option) { input.setAttribute('aria-activedescendant', option.id); option.scrollIntoView({block: 'nearest'}); }
    else input.removeAttribute('aria-activedescendant');
  }
  function choose(index: number): void {
    const item = items[index]; if (!item) return;
    close(); input.value = item.value; input.focus();
    // Filling a suggestion is a local edit, never a submit or server mutation.
    input.dispatchEvent(new Event('change', {bubbles: true}));
  }
  function render(query: string): void {
    list.replaceChildren(); active = -1; input.removeAttribute('aria-activedescendant');
    for (const [index, item] of items.entries()) {
      const option = document.createElement('div'); option.className = 'autocomplete-option'; option.id = `${list.id}-${index}`; option.setAttribute('role', 'option'); option.setAttribute('aria-selected', 'false');
      const title = document.createElement('strong');
      const offset = item.value.toLowerCase().indexOf(query.toLowerCase());
      if (offset >= 0) {
        const mark = document.createElement('mark'); mark.textContent = item.value.slice(offset, offset + query.length);
        title.append(document.createTextNode(item.value.slice(0, offset)), mark, document.createTextNode(item.value.slice(offset + query.length)));
      } else title.textContent = item.value;
      const detail = document.createElement('small'); detail.textContent = `${item.source === 'shared' ? '↔ 共享记录' : '✦ 常用建议'} · ${item.detail}`;
      option.append(title, detail);
      option.addEventListener('pointerdown', event => event.preventDefault());
      option.addEventListener('click', () => choose(index));
      option.addEventListener('pointermove', () => select(index));
      list.append(option);
    }
    show(items.length ? `${items.length} 条建议 · ↑↓ 选择 · Enter 填入 · Esc 关闭` : '暂无匹配，继续输入或直接提交新内容');
  }
  function schedule(): void {
    cancel(); items = []; list.replaceChildren(); active = -1; input.removeAttribute('aria-activedescendant');
    const query = input.value.trim();
    if (composing || !query || [...query].length > 120 || document.activeElement !== input) { close(); return; }
    show('正在查找建议…');
    const current = generation;
    timer = setTimeout(async () => {
      const controller = new AbortController(); pending = controller;
      try {
        const data = decodeSuggestions(await encryptedRequest({method: 'POST', path: '/api/suggestions', body: JSON.stringify({kind, query}), form: false}, controller.signal));
        if (generation !== current || composing || input.value.trim() !== query || document.activeElement !== input) return;
        if (data.query !== query) throw new Error('补全查询不匹配');
        items = data.items; render(query);
      } catch (error) {
        if (generation !== current || controller.signal.aborted) return;
        show('建议暂不可用，仍可直接输入并提交');
      } finally { if (pending === controller) pending = undefined; }
    }, 180);
  }
  input.addEventListener('input', schedule);
  input.addEventListener('focus', schedule);
  input.addEventListener('blur', close);
  input.addEventListener('compositionstart', () => { composing = true; close(); });
  input.addEventListener('compositionend', () => { composing = false; schedule(); });
  input.addEventListener('keydown', event => {
    if (composing || event.isComposing || event.keyCode === 229) return;
    if (event.key === 'Escape') { event.preventDefault(); close(); return; }
    if (event.key === 'Tab') { close(); return; }
    if (panel.hidden) { if (event.key === 'ArrowDown') { event.preventDefault(); schedule(); } return; }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (!items.length) return;
      event.preventDefault(); select(event.key === 'ArrowDown' ? (active + 1) % items.length : (active <= 0 ? items.length - 1 : active - 1));
    } else if (event.key === 'Enter' && active >= 0) { event.preventDefault(); choose(active); }
  });
  input.form?.addEventListener('submit', close);
  window.addEventListener('pagehide', close);
}
