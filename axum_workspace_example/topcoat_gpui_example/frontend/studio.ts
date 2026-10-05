import { element, errorMessage } from './api.ts';
import { studioApi, themes, isTheme } from './studio-api.ts';
import type { StudioSnapshot } from './studio-api.ts';
const theme = element('theme', HTMLSelectElement);
const intensity = element('intensity', HTMLInputElement);
const preview = element('preview', HTMLElement);
const draftStatus = element('draft-status', HTMLElement);
const status = element('status', HTMLElement);
const publish = element('publish', HTMLButtonElement);
const refresh = element('refresh', HTMLButtonElement);
let busy = false;
let dirty = false;
let latest: StudioSnapshot | undefined;
function renderPreview(): void {
  if (!isTheme(theme.value)) return;
  const selected = themes[theme.value];
  preview.style.backgroundColor = selected.color;
  preview.style.opacity = String(0.2 + intensity.valueAsNumber * 0.008);
  element('preview-label', HTMLElement).textContent = `${selected.label} · 强度 ${intensity.value}%`;
  draftStatus.textContent = dirty ? '本地预览 · 点击发布同步到桌面' : '正在展示已发布配色';
}
function showPublished(snapshot: StudioSnapshot): void {
  latest = snapshot;
  element('published', HTMLElement).textContent = `${themes[snapshot.theme].label} · 强度 ${snapshot.intensity}% · 版本 ${snapshot.revision}`;
  if (!dirty) {
    theme.value = snapshot.theme;
    intensity.value = String(snapshot.intensity);
    renderPreview();
  }
}
async function sync(save = false): Promise<void> {
  if (busy) return;
  busy = true;
  publish.disabled = refresh.disabled = theme.disabled = intensity.disabled = true;
  try {
    if (!isTheme(theme.value)) throw new Error('未知主题');
    const snapshot = save ? await studioApi.apply(theme.value, intensity.valueAsNumber) : await studioApi.read();
    if (save) dirty = false;
    showPublished(snapshot);
    status.textContent = '已同步 · 每两秒检查桌面修改';
  } catch (error) {
    status.textContent = `同步失败，显示数据可能已过期：${errorMessage(error)}。发布不会自动重试。`;
  } finally {
    busy = false;
    publish.disabled = refresh.disabled = theme.disabled = intensity.disabled = false;
  }
}
for (const control of [theme, intensity]) control.oninput = () => { dirty = true; renderPreview(); };
publish.onclick = () => void sync(true);
refresh.onclick = () => {
  dirty = false;
  if (latest) showPublished(latest);
  void sync();
};
renderPreview();
void sync();
setInterval(() => void sync(), 2000);
