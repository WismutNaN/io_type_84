import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/ui',
  use: { baseURL: 'http://127.0.0.1:1420', channel: 'msedge', headless: true },
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
  },
  workers: 1,
  reporter: 'list',
});
