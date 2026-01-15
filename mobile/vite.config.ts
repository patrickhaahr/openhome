import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";

// @ts-expect-error process is a nodejs global
const envCwd = process.cwd();
// @ts-expect-error process is a nodejs global
const processEnv = process.env;

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, envCwd, "");
  const host = env.TAURI_DEV_HOST || processEnv.TAURI_DEV_HOST;

  return {
    plugins: [solid()],

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
  };
});
