import * as Location from 'expo-location';
import * as TaskManager from 'expo-task-manager';

import type { HomeGeofence } from '../domain/home-geofence';
import { failure, success, type Result } from '../domain/result';
import type { HomeGeofenceStore } from './home-geofence-store';

/** The globally registered operating-system geofence task. */
export const homeGeofenceTaskName = 'openhome-home-geofence';

/** Operations required by the home-geofence application flow. */
export type HomeGeofenceService = {
  readonly load: () => Promise<Result<HomeGeofence | null>>;
  readonly setAtCurrentLocation: (radiusMeters: number) => Promise<Result<HomeGeofence>>;
  readonly disable: () => Promise<Result<void>>;
};

/** Create the Expo Location adapter for the single home geofence. */
export function createHomeGeofenceService(store: HomeGeofenceStore): HomeGeofenceService {
  async function register(home: HomeGeofence): Promise<void> {
    await Location.startGeofencingAsync(homeGeofenceTaskName, [{
      identifier: home.identifier,
      latitude: home.latitude,
      longitude: home.longitude,
      radius: home.radiusMeters,
      notifyOnEnter: false,
      notifyOnExit: true,
    }]);
  }

  async function restore(previous: HomeGeofence | null): Promise<void> {
    const restored = await store.save(previous);
    if (!restored.ok) {
      throw new Error(restored.error);
    }
    if (previous === null) {
      await stopMonitoring();
      return;
    }
    await register(previous);
  }

  async function stopMonitoring(): Promise<void> {
    if (await Location.hasStartedGeofencingAsync(homeGeofenceTaskName)) {
      await Location.stopGeofencingAsync(homeGeofenceTaskName);
    }
  }

  return {
    load: store.load,

    async setAtCurrentLocation(radiusMeters: number): Promise<Result<HomeGeofence>> {
      try {
        if (!await TaskManager.isAvailableAsync()) {
          return failure('Background location is unavailable in this build. Use an Android or iOS development build.');
        }

        const foreground = await Location.requestForegroundPermissionsAsync();
        if (!foreground.granted) {
          return failure('Allow location access to set your home location.');
        }
        if (foreground.android?.accuracy !== undefined && foreground.android.accuracy !== 'fine') {
          return failure('Allow precise location so OpenHome can monitor your home radius reliably.');
        }
        if (foreground.ios?.accuracy === 'reduced') {
          return failure('Enable Precise Location so OpenHome can monitor your home radius reliably.');
        }
        const background = await Location.requestBackgroundPermissionsAsync();
        if (!background.granted) {
          return failure('Allow background location so OpenHome can detect when you leave home.');
        }
      } catch {
        return failure("Couldn't request location access. Check location settings and try again.");
      }

      let location: Location.LocationObject;
      try {
        location = await Location.getCurrentPositionAsync({ accuracy: Location.Accuracy.High });
      } catch {
        return failure("Couldn't determine your current location. Check that location services are on and try again.");
      }
      if (location.coords.accuracy === null || location.coords.accuracy > radiusMeters) {
        return failure(`Location accuracy must be within the ${radiusMeters} meter radius. Move near a window and try again.`);
      }

      const previous = await store.load();
      if (!previous.ok) {
        return previous;
      }
      const home: HomeGeofence = {
        identifier: `home-${Date.now()}`,
        latitude: location.coords.latitude,
        longitude: location.coords.longitude,
        radiusMeters,
      };
      const saved = await store.save(home);
      if (!saved.ok) {
        return saved;
      }

      try {
        await register(home);
        return success(home);
      } catch {
        try {
          await restore(previous.value);
        } catch {
          return failure("Couldn't start monitoring or restore the previous home location. Disable home automation and try again.");
        }
        return failure("Couldn't start monitoring the home location.");
      }
    },

    async disable(): Promise<Result<void>> {
      const removed = await store.save(null);
      if (!removed.ok) {
        return removed;
      }

      try {
        await stopMonitoring();
      } catch {
        // The callback verifies persisted state, so removing it safely disables commands even if
        // revoked permissions prevent the operating system registration from being removed.
      }
      return success(undefined);
    },
  };
}
