import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { createSignal } from "solid-js";

export interface ApiConfig {
  base_url: string;
  timeout_seconds: number;
}

let store: Store | null = null;

const [baseUrl, setBaseUrl] = createSignal<string>("");
const [timeoutSeconds, setTimeoutSeconds] = createSignal<number>(30);
const [isLoaded, setIsLoaded] = createSignal(false);

const initStore = async () => {
  try {
    store = await Store.load("store.bin");

    const storedUrl = await store.get<string>("base_url");
    const storedTimeout = await store.get<number>("timeout_seconds");

    // Load default config from Rust (empty base_url, 30s timeout)
    const config = await invoke<ApiConfig>("get_api_config");

    if (storedUrl) {
      setBaseUrl(storedUrl);
    } else {
      setBaseUrl(config.base_url);
    }

    if (storedTimeout) {
      setTimeoutSeconds(storedTimeout);
    } else {
      setTimeoutSeconds(config.timeout_seconds);
    }

    setIsLoaded(true);
  } catch (e) {
    console.error("Failed to init store", e);
    setIsLoaded(true);
  }
};

initStore();

export const saveSettings = async (url: string, timeout: number) => {
  if (!store) {
    store = await Store.load("store.bin");
  }
  await store.set("base_url", url);
  await store.set("timeout_seconds", timeout);
  await store.save();

  setBaseUrl(url);
  setTimeoutSeconds(timeout);
};

export { baseUrl, timeoutSeconds, isLoaded };
