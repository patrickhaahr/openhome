import * as Location from "expo-location";
import * as TaskManager from "expo-task-manager";

import { failure, success } from "../domain/result";
import { homeGeofenceTaskName, type HomeGeofenceBackend } from "./home-geofence-service";

/** Create the existing Expo Location Home Geofence backend. */
export function createExpoHomeGeofenceBackend(): HomeGeofenceBackend {
  return {
    async checkAvailability() {
      try {
        if (!(await TaskManager.isAvailableAsync())) {
          return failure(
            "Background location is unavailable in this build. Use an Android or iOS development build.",
          );
        }
        const foreground = await Location.requestForegroundPermissionsAsync();
        if (!foreground.granted) {
          return failure("Allow location access to set your home location.");
        }
        if (foreground.android?.accuracy !== undefined && foreground.android.accuracy !== "fine") {
          return failure(
            "Allow precise location so OpenHome can monitor your home radius reliably.",
          );
        }
        if (foreground.ios?.accuracy === "reduced") {
          return failure(
            "Enable Precise Location so OpenHome can monitor your home radius reliably.",
          );
        }
        const background = await Location.requestBackgroundPermissionsAsync();
        if (!background.granted) {
          return failure("Allow background location so OpenHome can detect when you leave home.");
        }
        return success(undefined);
      } catch {
        return failure("Couldn't request location access. Check location settings and try again.");
      }
    },

    async getCurrentPosition() {
      try {
        const location = await Location.getCurrentPositionAsync({
          accuracy: Location.Accuracy.High,
        });
        return success({
          latitude: location.coords.latitude,
          longitude: location.coords.longitude,
          accuracyMeters: location.coords.accuracy,
        });
      } catch {
        return failure(
          "Couldn't determine your current location. Check that location services are on and try again.",
        );
      }
    },

    async start(home) {
      await Location.startGeofencingAsync(homeGeofenceTaskName, [
        {
          identifier: home.identifier,
          latitude: home.latitude,
          longitude: home.longitude,
          radius: home.radiusMeters,
          notifyOnEnter: false,
          notifyOnExit: true,
        },
      ]);
    },

    async stop() {
      try {
        if (await Location.hasStartedGeofencingAsync(homeGeofenceTaskName)) {
          await Location.stopGeofencingAsync(homeGeofenceTaskName);
        }
      } catch {
        // expo-location checks background permission before unregistering its persisted task.
        // TaskManager can still forget it after permission has been revoked.
        if (await TaskManager.isTaskRegisteredAsync(homeGeofenceTaskName)) {
          await TaskManager.unregisterTaskAsync(homeGeofenceTaskName);
        }
      }
    },
  };
}
