/// <reference types="vitest" />
import { vi } from 'vitest';

// Create a single mock store instance to use
const createMockStore = () => ({
  load: vi.fn().mockResolvedValue({}),
  get: vi.fn().mockResolvedValue(null),
  set: vi.fn().mockResolvedValue(undefined),
  save: vi.fn().mockResolvedValue(undefined),
});

// Mock Tauri core APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({
    base_url: 'http://localhost:8080',
    timeout_seconds: 30
  }),
}));

// Mock Tauri plugin store with proper instance
vi.mock('@tauri-apps/plugin-store', () => {
  return {
    Store: vi.fn().mockImplementation(() => createMockStore()),
  };
});

(global as any).window ??= {} as any;
(global as any).window.__TAURI_INTERNALS__ = { invoke: vi.fn() };

// Set up test environment variables
vi.stubEnv('VITE_TEST', 'true');