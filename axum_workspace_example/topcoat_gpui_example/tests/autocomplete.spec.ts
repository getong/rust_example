import { test, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
const exec = promisify(execFile);
const native = process.env.TOPCOAT_AUTOCOMPLETE_SMOKE ?? fileURLToPath(new URL('../../../gpui_workspace_example/target/debug/topcoat-smoke', import.meta.url));

test('one-character completion, keyboard/mouse, aliases, shared native history and encrypted transport', async ({page, baseURL}) => {
  const endpoints: string[] = [];
  page.on('request', request => { if (/^\/(api|pq)\//.test(new URL(request.url()).pathname)) endpoints.push(new URL(request.url()).pathname); });
  await page.goto('/todos');
  await expect(page.locator('#status')).toContainText('已同步');
  const input = page.getByRole('combobox');
  await input.fill('学');
  await expect(page.getByRole('option').first()).toContainText('学习');
  expect(await page.getByRole('option').count()).toBeLessThanOrEqual(6);
  await input.press('ArrowDown');
  await expect(page.getByRole('option').first()).toHaveAttribute('aria-selected','true');
  await input.press('Enter');
  await expect(input).toHaveValue('学习 Rust 异步编程');
  await expect(input).toHaveAttribute('aria-expanded','false');
  await expect(page.locator('#results')).not.toContainText('学习 Rust 异步编程');
  await input.fill('kf');
  await page.getByRole('option').filter({hasText:'开发实时搜索补全'}).click();
  await expect(input).toHaveValue('开发实时搜索补全');
  await input.fill('学');
  await expect(page.getByRole('option').first()).toBeVisible();
  await input.press('Escape');
  await expect(page.getByRole('listbox')).toBeHidden();
  await input.fill('zzzz-no-match');
  await expect(page.locator('.autocomplete-status')).toContainText('暂无匹配');
  await input.fill('');
  await expect(page.getByRole('listbox')).toBeHidden();
  // Actual native client writes shared state; browser reads suggestions from the same store.
  await exec(native, [], {env: {...process.env, TOPCOAT_URL: baseURL!}, timeout: 30000});
  await input.fill('GPUI');
  await expect(page.getByRole('option').filter({hasText:'GPUI task'})).toContainText('共享记录');
  expect(endpoints.every(path => path === '/pq/handshake' || path === '/pq/exchange')).toBe(true);
  await page.goto('/profile');
  await page.getByRole('combobox').fill('zxm');
  await page.getByRole('option').filter({hasText:'张小明'}).click();
  await expect(page.getByRole('combobox')).toHaveValue('张小明');
  await expect(page.locator('#age')).toHaveValue('18');
});

test('IME composition, stale requests, blur and unavailable suggestions preserve free typing', async ({page}) => {
  await page.goto('/todos');
  await expect(page.locator('#status')).toContainText('已同步');
  const input = page.getByRole('combobox');
  await input.focus();
  await input.dispatchEvent('compositionstart');
  await input.fill('xue');
  await expect(page.getByRole('listbox')).toBeHidden();
  await input.fill('学');
  await input.dispatchEvent('compositionend');
  await expect(page.getByRole('option').first()).toContainText('学习');
  // Delay one response. A replacement query must own the visible dropdown.
  let delayed = false;
  await page.route('**/pq/exchange', async route => {
    if (delayed) { await route.continue(); return; }
    delayed = true;
    const response = await route.fetch();
    await new Promise(resolve => setTimeout(resolve, 700));
    try { await route.fulfill({response}); } catch { /* Superseded request was aborted. */ }
  });
  await input.fill('rust');
  await expect.poll(() => delayed).toBe(true);
  await input.fill('开发');
  await expect(page.getByRole('option').first()).toContainText('开发');
  await page.waitForTimeout(850);
  await expect(page.getByRole('option').first()).toContainText('开发');
  await page.unroute('**/pq/exchange');
  await page.locator('h1').click();
  await expect(page.getByRole('listbox')).toBeHidden();
  await page.route('**/pq/handshake', route => route.abort());
  await input.fill('自由输入');
  await expect(page.locator('.autocomplete-status')).toContainText('暂不可用');
  await expect(input).toHaveValue('自由输入');
  await page.unroute('**/pq/handshake');
  await input.fill('学');
  await expect(page.getByRole('option').first()).toContainText('学习');
});
