import type { Component } from "solid-js";
import { createSignal, createEffect, on, Show } from "solid-js";
import { saveSettings, saveApiKey, baseUrl, timeoutSeconds, isLoaded } from "../stores/config";
import { getKeyringStatus, type KeyringDiagnostics } from "../api";
import { getHealthStatus } from "../api/health";

interface SettingsProps {
  onSaved: () => void;
}

const Settings: Component<SettingsProps> = (props) => {
  const [localBaseUrl, setLocalBaseUrl] = createSignal("");
  const [localTimeout, setLocalTimeout] = createSignal(30);
  const [localKey, setLocalKey] = createSignal("");
  const [saveStatus, setSaveStatus] = createSignal("");
  
  const [showDebug, setShowDebug] = createSignal(false);
  const [debugKey, setDebugKey] = createSignal("");
  const [debugStatus, setDebugStatus] = createSignal("");
  const [keyringInfo, setKeyringInfo] = createSignal<KeyringDiagnostics | null>(null);

  createEffect(on([isLoaded], () => {
    if (isLoaded()) {
      setLocalBaseUrl(baseUrl());
      setLocalTimeout(timeoutSeconds());
      setSaveStatus("");
      setLocalKey("");
    }
  }));

  const handleSave = async () => {
    const url = localBaseUrl();
    const timeout = localTimeout();
    
    if (!url.trim()) {
      setSaveStatus("Base URL required");
      return;
    }
    if (timeout < 1 || timeout > 300) {
      setSaveStatus("Timeout must be 1-300s");
      return;
    }

    try {
      await saveSettings(url, timeout);
      if (localKey()) {
        await saveApiKey(localKey());
        setLocalKey("");
      }
      setSaveStatus("Saved!");
      setTimeout(() => setSaveStatus(""), 2000);
      props.onSaved();
    } catch (e) {
      console.error(e);
      setSaveStatus("Error saving");
    }
  };

  const isError = () => {
    const status = saveStatus();
    return status && status !== "Saved!";
  };

  const handleTestConnection = async () => {
    const key = debugKey();
    if (!key.trim()) {
      setDebugStatus("Enter a key to test");
      return;
    }
    setDebugStatus("Testing...");
    try {
      const result = await getHealthStatus(key);
      setDebugStatus(`Success: ${result.status}`);
    } catch (e: unknown) {
      setDebugStatus(`Failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleSaveToKeyring = async () => {
    const key = debugKey();
    if (!key.trim()) {
      setDebugStatus("Enter a key to save");
      return;
    }
    try {
      await saveApiKey(key);
      setDebugKey("");
      setDebugStatus("Saved to keyring!");
      await refreshKeyringStatus();
    } catch (e: unknown) {
      setDebugStatus(`Save failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const refreshKeyringStatus = async () => {
    try {
      const info = await getKeyringStatus();
      setKeyringInfo(info);
    } catch {
      setKeyringInfo(null);
    }
  };

  return (
    <div class="rounded-xl bg-bg-secondary p-4 sm:p-6">
      <h2 class="mb-6 text-lg font-medium text-text-primary">Settings</h2>
      
      <div class="space-y-5">
        <div class="space-y-2">
          <label class="block text-sm font-medium text-text-secondary">Base URL</label>
          <input 
            type="text" 
            value={localBaseUrl()} 
            onInput={(e) => setLocalBaseUrl(e.currentTarget.value)}
            placeholder="https://api.example.com"
            class="w-full rounded-lg border border-border bg-bg-tertiary px-4 py-3 text-text-primary placeholder-text-muted transition-colors focus:border-accent focus:outline-none"
          />
        </div>

        <div class="space-y-2">
          <label class="block text-sm font-medium text-text-secondary">Timeout (seconds)</label>
          <input 
            type="number" 
            min="1" 
            max="300"
            value={localTimeout()} 
            onInput={(e) => setLocalTimeout(Number(e.currentTarget.value))}
            class="w-full rounded-lg border border-border bg-bg-tertiary px-4 py-3 text-text-primary placeholder-text-muted transition-colors focus:border-accent focus:outline-none"
          />
        </div>

        <div class="space-y-2">
          <label class="block text-sm font-medium text-text-secondary">API Key</label>
          <input 
            type="password" 
            value={localKey()} 
            onInput={(e) => setLocalKey(e.currentTarget.value)}
            placeholder="Enter new key to update"
            class="w-full rounded-lg border border-border bg-bg-tertiary px-4 py-3 text-text-primary placeholder-text-muted transition-colors focus:border-accent focus:outline-none"
          />
          <p class="text-xs text-text-muted">Leave empty to keep existing key</p>
        </div>

        <div class="flex flex-col gap-3 pt-2 sm:flex-row sm:items-center">
          <button 
            onClick={handleSave} 
            class="w-full rounded-lg bg-accent px-4 py-3 font-medium text-white transition-colors hover:bg-accent-hover sm:w-auto"
          >
            Save
          </button>
          <span 
            class={`text-sm ${isError() ? "text-error" : "text-success"}`}
          >
            {saveStatus()}
          </span>
        </div>

        <Show when={import.meta.env.DEV}>
          <div class="mt-6 border-t border-border pt-6">
            <button
              onClick={() => {
                setShowDebug(!showDebug());
                if (!showDebug()) {
                  refreshKeyringStatus();
                }
              }}
              class="text-sm text-text-muted hover:text-text-secondary"
            >
              {showDebug() ? "Hide Debug Tools" : "Show Debug Tools"}
            </button>

            <Show when={showDebug()}>
              <div class="mt-4 space-y-4 rounded-lg bg-bg-tertiary p-4">
                <h3 class="text-sm font-medium text-text-primary">Debug Tools</h3>

                <div class="space-y-2">
                  <label class="block text-xs text-text-muted">Manual API Key (ephemeral)</label>
                  <input
                    type="password"
                    value={debugKey()}
                    onInput={(e) => setDebugKey(e.currentTarget.value)}
                    placeholder="Enter key for testing"
                    class="w-full rounded border border-border bg-bg-secondary px-3 py-2 text-sm text-text-primary placeholder-text-muted focus:border-accent focus:outline-none"
                  />
                </div>

                <div class="flex flex-wrap gap-2">
                  <button
                    onClick={handleTestConnection}
                    class="rounded bg-accent/80 px-3 py-1.5 text-xs font-medium text-white hover:bg-accent"
                  >
                    Test Connection
                  </button>
                  <button
                    onClick={handleSaveToKeyring}
                    class="rounded bg-success/80 px-3 py-1.5 text-xs font-medium text-white hover:bg-success"
                  >
                    Save to Keyring
                  </button>
                  <button
                    onClick={refreshKeyringStatus}
                    class="rounded bg-text-muted/20 px-3 py-1.5 text-xs font-medium text-text-secondary hover:bg-text-muted/30"
                  >
                    Refresh Status
                  </button>
                </div>

                <Show when={debugStatus()}>
                  <p class="text-xs text-text-muted">{debugStatus()}</p>
                </Show>

                <Show when={keyringInfo()}>
                  {(info) => (
                    <div class="mt-2 space-y-1 text-xs text-text-muted">
                      <p>Keyring accessible: {info().keyring_accessible ? "Yes" : "No"}</p>
                      <p>Key present: {info().key_present ? "Yes" : "No"}</p>
                      <Show when={info().key_length !== null}>
                        <p>Key length: {info().key_length}</p>
                      </Show>
                    </div>
                  )}
                </Show>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default Settings;
