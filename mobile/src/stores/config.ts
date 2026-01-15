import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { createResource, createSignal, createMemo } from "solid-js";

export interface ApiConfig {
  base_url: string;
  timeout_seconds: number;
}

let store: Store | null = null;

const [apiKey, setApiKey] = createSignal<string | null>(null);

const [backendConfig] = createResource(async () => {
  try {
    const config = await invoke<ApiConfig>("get_api_config");
    console.log("Loaded config:", config);
    return config;
  } catch (e) {
    console.error("Failed to load backend config", e);
    throw e;
  }
});

const baseUrl = createMemo(() => backendConfig()?.base_url ?? "");
const timeoutSeconds = createMemo(() => backendConfig()?.timeout_seconds ?? 30);

const initStore = async () => {
  try {
    store = await Store.load("store.bin");
    const val = await store.get<string>("api_key");
    if (val) {
      setApiKey(val);
    }
  } catch (e) {
    console.error("Failed to load API key from store", e);
  }
};

initStore();

export const saveApiKey = async (key: string) => {
  try {
    if (!store) {
      store = await Store.load("store.bin");
    }
    await store.set("api_key", key);
    await store.save();
    setApiKey(key);
  } catch (e) {
    console.error("Failed to save API key", e);
  }
};

export { apiKey, backendConfig, baseUrl, timeoutSeconds };
