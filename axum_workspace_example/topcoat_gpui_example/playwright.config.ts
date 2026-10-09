import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests', testMatch: '*.spec.ts', workers: 1,
  use: { baseURL: 'http://127.0.0.1:3198' },
  webServer: {
    command: 'bun scripts/e2e-server.ts',
    url: 'http://127.0.0.1:3198/',
    env: { HOST: '127.0.0.1', PORT: '3198' },
    reuseExistingServer: false, timeout: 120000,
  },
});
