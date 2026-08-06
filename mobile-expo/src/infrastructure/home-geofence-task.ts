import { GeofencingEventType, GeofencingRegionState, type LocationRegion } from 'expo-location';
import * as TaskManager from 'expo-task-manager';

import { handleHomeGeofenceEvent, retryPendingHomeExit } from '../application/handle-home-geofence-event';
import { createSecureConfigurationStore } from './configuration-store';
import { homeGeofenceTaskName } from './home-geofence-service';
import { createHomeGeofenceStore } from './home-geofence-store';
import { createOpenHomeApi } from './open-home-api';

type HomeGeofenceTaskData = {
  readonly eventType: GeofencingEventType;
  readonly region: LocationRegion;
};

const configurationStore = createSecureConfigurationStore();
const homeStore = createHomeGeofenceStore();
const dependencies = {
  loadConfiguration: configurationStore.load,
  loadHome: homeStore.load,
  loadPending: homeStore.loadPending,
  savePending: homeStore.savePending,
  turnOffLights: async (configuration: Parameters<typeof createOpenHomeApi>[0]) => createOpenHomeApi(configuration).sendLightCommand('off'),
};

if (!TaskManager.isTaskDefined(homeGeofenceTaskName)) {
  TaskManager.defineTask<HomeGeofenceTaskData>(homeGeofenceTaskName, async ({ data, error }) => {
    if (error !== null) {
      console.error(`Home geofence failed: ${error.message}`);
      return;
    }

    const result = await handleHomeGeofenceEvent(
      {
        type: data.eventType === GeofencingEventType.Exit ? 'exit' : 'enter',
        regionIdentifier: data.region.identifier ?? '',
        regionState: geofenceRegionState(data.region.state),
      },
      dependencies,
    );
    if (!result.ok) {
      console.error(`Home exit could not turn off the lights: ${result.error}`);
    }
  });
}

/** Retry a light-off command persisted by a failed background geofence event. */
export async function retryPendingHomeExitCommand(): Promise<void> {
  const result = await retryPendingHomeExit(dependencies);
  if (!result.ok) {
    console.error(`Pending home exit could not turn off the lights: ${result.error}`);
  }
}

function geofenceRegionState(state: LocationRegion['state']): 'inside' | 'outside' | 'unknown' {
  if (state === GeofencingRegionState.Inside) {
    return 'inside';
  }
  if (state === GeofencingRegionState.Outside) {
    return 'outside';
  }
  return 'unknown';
}
