import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  use: {
    baseURL: process.env.IACA_TEST_URL ?? 'http://127.0.0.1:8787',
    viewport: { width: 1440, height: 1000 },
    launchOptions: process.env.IACA_CHROME_PATH ? { executablePath: process.env.IACA_CHROME_PATH } : {},
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
})
