import * as Location from "expo-location";
import { Platform } from "react-native";

import { failure, success } from "../domain/result";
import { nativeHomeGeofenceTaskName, type HomeGeofenceBackend } from "./home-geofence-service";
import { nativeHomeGeofenceModule } from "./native-home-geofence-module";

/** Create the Android LocationManager Home Geofence backend. */
export function createNativeHomeGeofenceBackend(): HomeGeofenceBackend {
  return {
    async checkAvailability() {
      if (Platform.OS !== "android" || nativeHomeGeofenceModule === null) {
        return failure(
          "Native location is available only in an Android development or release build.",
        );
      }
      try {
        const foreground = await Location.requestForegroundPermissionsAsync();
        if (!foreground.granted) {
          return failure("Allow location access to set your home location.");
        }
        if (foreground.android?.accuracy !== "fine") {
          return failure(
            "Allow precise location so OpenHome can monitor your home radius reliably.",
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
      if (nativeHomeGeofenceModule === null) {
        return failure("Native location is unavailable in this build.");
      }
      try {
        return success(await nativeHomeGeofenceModule.getCurrentPositionAsync());
      } catch {
        return failure(
          "Couldn't determine your current location. Check that location services are on and try again.",
        );
      }
    },

    async start(home) {
      if (nativeHomeGeofenceModule === null) {
        throw new Error("Native location is unavailable in this build.");
      }
      await nativeHomeGeofenceModule.startMonitoringAsync(nativeHomeGeofenceTaskName, home);
    },

    async stop() {
      if (
        nativeHomeGeofenceModule !== null &&
        (await nativeHomeGeofenceModule.hasStartedMonitoringAsync(nativeHomeGeofenceTaskName))
      ) {
        await nativeHomeGeofenceModule.stopMonitoringAsync(nativeHomeGeofenceTaskName);
      }
    },
  };
}
