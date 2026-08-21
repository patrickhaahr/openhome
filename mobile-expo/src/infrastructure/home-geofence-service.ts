import type { HomeGeofence, HomeGeofenceProvider } from "../domain/home-geofence";
import { failure, success, type Result } from "../domain/result";
import type { HomeGeofenceStore } from "./home-geofence-store";

/** The globally registered operating-system geofence task. */
export const homeGeofenceTaskName = "openhome-home-geofence";

/** The Android framework Home Geofence task. */
export const nativeHomeGeofenceTaskName = "openhome-native-home-geofence";

/** A current position returned by a location backend. */
export type HomePosition = {
  readonly latitude: number;
  readonly longitude: number;
  readonly accuracyMeters: number | null;
};

/** Operating-system operations needed by one Home Geofence provider. */
export type HomeGeofenceBackend = {
  readonly checkAvailability: () => Promise<Result<void>>;
  readonly getCurrentPosition: () => Promise<Result<HomePosition>>;
  readonly start: (home: HomeGeofence) => Promise<void>;
  readonly stop: () => Promise<void>;
};

/** Operations required by the Home Geofence application flow. */
export type HomeGeofenceService = {
  readonly load: () => Promise<Result<HomeGeofence | null>>;
  readonly resume: () => Promise<Result<void>>;
  readonly setAtCurrentLocation: (
    radiusMeters: number,
    provider: HomeGeofenceProvider,
  ) => Promise<Result<HomeGeofence>>;
  readonly disable: () => Promise<Result<void>>;
};

type Backends = Readonly<Record<HomeGeofenceProvider, HomeGeofenceBackend>>;

/** Create the provider-selecting module for the single Home Geofence. */
export function createHomeGeofenceService(
  store: HomeGeofenceStore,
  backends: Backends,
): HomeGeofenceService {
  async function restore(
    previous: HomeGeofence | null,
    attemptedBackend: HomeGeofenceBackend,
  ): Promise<void> {
    await attemptedBackend.stop();
    const restored = await store.save(previous);
    if (!restored.ok) {
      throw new Error(restored.error);
    }
    if (previous !== null) {
      await backends[previous.provider].start(previous);
    }
  }

  return {
    load: store.load,

    async resume(): Promise<Result<void>> {
      const home = await store.load();
      if (!home.ok || home.value === null) {
        return home.ok ? success(undefined) : home;
      }
      try {
        await backends[home.value.provider].start(home.value);
        return success(undefined);
      } catch {
        return failure("Couldn't resume monitoring the home location.");
      }
    },

    async setAtCurrentLocation(radiusMeters, provider): Promise<Result<HomeGeofence>> {
      const backend = backends[provider];
      const availability = await backend.checkAvailability();
      if (!availability.ok) {
        return availability;
      }
      const position = await backend.getCurrentPosition();
      if (!position.ok) {
        return position;
      }
      const { latitude, longitude, accuracyMeters } = position.value;
      if (accuracyMeters === null || accuracyMeters > radiusMeters) {
        return failure(
          `Location accuracy must be within the ${radiusMeters} meter radius. Move near a window and try again.`,
        );
      }

      const previous = await store.load();
      if (!previous.ok) {
        return previous;
      }
      const home: HomeGeofence = {
        identifier: `home-${Date.now()}`,
        latitude,
        longitude,
        radiusMeters,
        provider,
      };
      const saved = await store.save(home);
      if (!saved.ok) {
        return saved;
      }

      try {
        await backend.start(home);
        if (previous.value !== null && previous.value.provider !== provider) {
          await backends[previous.value.provider].stop();
        }
        return success(home);
      } catch {
        try {
          await restore(previous.value, backend);
        } catch {
          return failure(
            "Couldn't start monitoring or restore the previous home location. Disable home automation and try again.",
          );
        }
        return failure("Couldn't start monitoring the home location.");
      }
    },

    async disable(): Promise<Result<void>> {
      const previous = await store.load();
      if (!previous.ok) {
        return previous;
      }
      if (previous.value !== null) {
        try {
          await backends[previous.value.provider].stop();
        } catch {
          return failure("Couldn't stop monitoring the home location.");
        }
      }
      const removed = await store.save(null);
      if (!removed.ok) {
        if (previous.value !== null) {
          try {
            await backends[previous.value.provider].start(previous.value);
          } catch {
            return failure(
              "Couldn't persist disabling home automation or restore monitoring. Try setting home again.",
            );
          }
        }
        return removed;
      }
      return success(undefined);
    },
  };
}
