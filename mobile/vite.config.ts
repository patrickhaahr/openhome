/// <reference types="vitest" />
import { defineConfig, loadEnv } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

const envCwd = process.cwd();
const processEnv = process.env;

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, envCwd, "");
  const host = env.TAURI_DEV_HOST || processEnv.TAURI_DEV_HOST;

  return {
    plugins: [solid(), tailwindcss()],

    // ShadCN UI path alias
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },

    test: {
      environment: 'jsdom',
      include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
      globals: true,
      setupFiles: ['src/test-setup.ts'],
    },
  };
});