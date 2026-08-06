import * as SecureStore from 'expo-secure-store';

import { parseConfiguration, type Configuration } from '../domain/configuration';
import { failure, success, type Result } from '../domain/result';

const configurationKey = 'openhome.configuration';
const secureStoreOptions = { keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK };

/** Persistent storage required by the setup flow. */
export type ConfigurationStore = {
  readonly load: () => Promise<Result<Configuration | null>>;
  readonly save: (configuration: Configuration) => Promise<Result<void>>;
};

/** Create encrypted configuration storage backed by the platform keystore. */
export function createSecureConfigurationStore(): ConfigurationStore {
  return {
    async load(): Promise<Result<Configuration | null>> {
      try {
        const stored = await SecureStore.getItemAsync(configurationKey, secureStoreOptions);
        if (stored === null) {
          return success(null);
        }

        const parsed: unknown = JSON.parse(stored);
        if (!isStoredConfiguration(parsed)) {
          return failure('The saved configuration could not be read. Enter it again.');
        }
        return parseConfiguration(parsed.baseUrl, parsed.apiKey);
      } catch {
        return failure('The saved configuration could not be read. Enter it again.');
      }
    },

    async save(configuration: Configuration): Promise<Result<void>> {
      try {
        await SecureStore.setItemAsync(configurationKey, JSON.stringify(configuration), secureStoreOptions);
        return success(undefined);
      } catch {
        return failure("Couldn't persist configuration.");
      }
    },
  };
}

function isStoredConfiguration(value: unknown): value is Configuration {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  return 'baseUrl' in value && typeof value.baseUrl === 'string' && 'apiKey' in value && typeof value.apiKey === 'string';
}
