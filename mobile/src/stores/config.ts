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
const [apiKey, setApiKey] = createSignal<string>("");
const [isLoaded, setIsLoaded] = createSignal(false);

const initStore = async () => {
  try {
    store = await Store.load("store.bin");
    
    const storedUrl = await store.get<string>("base_url");
    const storedTimeout = await store.get<number>("timeout_seconds");

    // Load static defaults to fill gaps
    try {
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
    } catch (e) {
      console.error("Failed to load static config", e);
      // If static fails, keep existing or defaults (empty/30)
      if (storedUrl) setBaseUrl(storedUrl);
      if (storedTimeout) setTimeoutSeconds(storedTimeout);
    }

    // Load API key from secure storage
    try {
      const key = await invoke<string | null>("get_api_key");
      if (key) {
        setApiKey(key);
      }
    } catch (e) {
      console.error("Failed to load API key", e);
    }
    
    setIsLoaded(true);
  } catch (e) {
    console.error("Failed to init store", e);
    // Attempt to load static at least
    try {
      const config = await invoke<ApiConfig>("get_api_config");
      setBaseUrl(config.base_url);
      setTimeoutSeconds(config.timeout_seconds);
      setIsLoaded(true);
    } catch (e2) {
      console.error("Fatal config load error", e2);
    }
  }
};

initStore();

export const saveApiKey = async (key: string) => {
  await invoke("save_api_key", { key });
};

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

export { baseUrl, timeoutSeconds, apiKey, isLoaded };
