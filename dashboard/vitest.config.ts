import { defineConfig } from "vitest/config"

export default defineConfig({
  resolve: { tsconfigPaths: true },
  test: { testTimeout: 20000, hookTimeout: 30000 },
})
