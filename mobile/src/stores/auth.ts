import { invoke } from "@tauri-apps/api/core";
import { createStore } from "solid-js/store";

const AUTH_TIMEOUT_MINUTES = 5;

type ApiKeyStatus = "NotSet" | "Locked" | "Unlocked";

interface AuthState {
  status: ApiKeyStatus;
  isLoading: boolean;
}

const [state, setState] = createStore<AuthState>({
  status: "NotSet",
  isLoading: false,
});

const loadStatus = async (): Promise<void> => {
  setState("isLoading", true);
  try {
    const status = await invoke<ApiKeyStatus>("get_api_key_status");
    setState("status", status);
  } catch (e) {
    console.error("Failed to load API key status:", e);
    setState("status", "NotSet");
  } finally {
    setState("isLoading", false);
  }
};

const unlock = async (): Promise<void> => {
  setState("isLoading", true);
  try {
    await invoke("biometric_resume_auth", { timeoutMinutes: AUTH_TIMEOUT_MINUTES });
    await loadStatus();
  } catch (e) {
    console.error("Failed to unlock:", e);
    throw e;
  } finally {
    setState("isLoading", false);
  }
};

const reset = async (): Promise<void> => {
  setState("isLoading", true);
  try {
    await invoke("reset_api_key");
    await loadStatus();
  } catch (e) {
    console.error("Failed to reset:", e);
    throw e;
  } finally {
    setState("isLoading", false);
  }
};

const isUnlocked = () => state.status === "Unlocked";
const isLocked = () => state.status === "Locked";
const notSet = () => state.status === "NotSet";

export const auth = {
  get status() {
    return state.status;
  },
  get isLoading() {
    return state.isLoading;
  },
  isUnlocked,
  isLocked,
  notSet,
  loadStatus,
  unlock,
  reset,
};
