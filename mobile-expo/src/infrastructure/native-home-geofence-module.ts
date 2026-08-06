import { requireOptionalNativeModule } from 'expo-modules-core';

import type { HomeGeofence } from '../domain/home-geofence';
import type { HomePosition } from './home-geofence-service';

type NativeHomeGeofenceModule = {
  readonly getCurrentPositionAsync: () => Promise<HomePosition>;
  readonly hasStartedMonitoringAsync: (taskName: string) => Promise<boolean>;
  readonly startMonitoringAsync: (taskName: string, home: HomeGeofence) => Promise<void>;
  readonly stopMonitoringAsync: (taskName: string) => Promise<void>;
};

/** Optional Android AOSP Home Geofence native module. */
export const nativeHomeGeofenceModule = requireOptionalNativeModule<NativeHomeGeofenceModule>('ExpoNativeHomeGeofence');
