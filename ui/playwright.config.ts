import { defineConfig } from '@playwright/test'

// E2E runs against the real Rust core through the HTTP bridge (kintree-server serving the built UI).
export default defineConfig({
  testDir: './e2e',
  timeout: 20_000,
  fullyParallel: false,
  workers: 1,
  // one retry on CI only; locally a failure is always reported so regressions are not masked
  retries: process.env.CI ? 1 : 0,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:8787',
    trace: 'retain-on-failure',
    launchOptions: { executablePath: process.env.CHROMIUM_PATH || undefined },
  },
  webServer: {
    command: 'cd .. && cargo run -q -p kintree-app --bin kintree-server -- --port 8787 --static ui/dist',
    url: 'http://127.0.0.1:8787',
    reuseExistingServer: !process.env.CI,
    timeout: 240_000,
  },
})
