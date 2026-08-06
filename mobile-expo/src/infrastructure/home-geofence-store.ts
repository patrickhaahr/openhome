import * as SecureStore from 'expo-secure-store';

import { parseHomeGeofence, type HomeGeofence } from '../domain/home-geofence';
import { failure, success, type Result } from '../domain/result';

const homeGeofenceKey = 'openhome.home-geofence';
const pendingHomeExitKey = 'openhome.pending-home-exit';
const secureStoreOptions = { keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK };

/** Persistent storage for the active home geofence. */
export type HomeGeofenceStore = {
  readonly load: () => Promise<Result<HomeGeofence | null>>;
  readonly loadPending: () => Promise<Result<string | null>>;
  readonly save: (home: HomeGeofence | null) => Promise<Result<void>>;
  readonly savePending: (regionIdentifier: string | null) => Promise<Result<void>>;
};

/** Create encrypted home-geofence storage backed by the platform keystore. */
export function createHomeGeofenceStore(): HomeGeofenceStore {
  return {
    async load(): Promise<Result<HomeGeofence | null>> {
      try {
        const stored = await SecureStore.getItemAsync(homeGeofenceKey, secureStoreOptions);
        if (stored === null) {
          return success(null);
        }
        return parseHomeGeofence(JSON.parse(stored));
      } catch {
        return failure('The saved home location could not be read. Set it again.');
      }
    },

    async save(home: HomeGeofence | null): Promise<Result<void>> {
      try {
        if (home === null) {
          await SecureStore.deleteItemAsync(homeGeofenceKey, secureStoreOptions);
        } else {
          await SecureStore.setItemAsync(homeGeofenceKey, JSON.stringify(home), secureStoreOptions);
        }
        return success(undefined);
      } catch {
        return failure("Couldn't persist the home location.");
      }
    },

    async loadPending(): Promise<Result<string | null>> {
      try {
        return success(await SecureStore.getItemAsync(pendingHomeExitKey, secureStoreOptions));
      } catch {
        return failure("Couldn't read the pending home-exit command.");
      }
    },

    async savePending(regionIdentifier: string | null): Promise<Result<void>> {
      try {
        if (regionIdentifier !== null) {
          await SecureStore.setItemAsync(pendingHomeExitKey, regionIdentifier, secureStoreOptions);
        } else {
          await SecureStore.deleteItemAsync(pendingHomeExitKey, secureStoreOptions);
        }
        return success(undefined);
      } catch {
        return failure("Couldn't persist the pending home-exit command.");
      }
    },
  };
}
