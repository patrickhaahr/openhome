import type { Configuration } from "../domain/configuration";
import type { HomeGeofence } from "../domain/home-geofence";
import { success, type Result } from "../domain/result";

type Dependencies = {
  readonly loadConfiguration: () => Promise<Result<Configuration | null>>;
  readonly loadHome: () => Promise<Result<HomeGeofence | null>>;
  readonly loadPending: () => Promise<Result<string | null>>;
  readonly savePending: (regionIdentifier: string | null) => Promise<Result<void>>;
  readonly turnOffLights: (configuration: Configuration) => Promise<Result<void>>;
};

/** A normalized operating-system geofence event. */
export type HomeGeofenceEvent = {
  readonly type: "enter" | "exit";
  readonly regionIdentifier: string;
  readonly regionState: "inside" | "outside" | "unknown";
};

/** Turn off the lights for an operating-system home geofence exit event. */
export async function handleHomeGeofenceEvent(
  event: HomeGeofenceEvent,
  dependencies: Dependencies,
): Promise<Result<void>> {
  if (event.type !== "exit" || event.regionState !== "outside") {
    return success(undefined);
  }

  const home = await dependencies.loadHome();
  if (!home.ok) {
    return home;
  }
  if (home.value === null || home.value.identifier !== event.regionIdentifier) {
    return success(undefined);
  }

  const pending = await dependencies.savePending(home.value.identifier);
  if (!pending.ok) {
    return pending;
  }
  return retryPendingHomeExit(dependencies);
}

/** Retry a persisted light-off command after a transient background failure. */
export async function retryPendingHomeExit(dependencies: Dependencies): Promise<Result<void>> {
  const pending = await dependencies.loadPending();
  if (!pending.ok) {
    return pending;
  }
  if (pending.value === null) {
    return success(undefined);
  }
  const home = await dependencies.loadHome();
  if (!home.ok) {
    return home;
  }
  if (home.value === null || home.value.identifier !== pending.value) {
    return dependencies.savePending(null);
  }
  return sendPendingHomeExit(dependencies);
}

async function sendPendingHomeExit(dependencies: Dependencies): Promise<Result<void>> {
  const configuration = await dependencies.loadConfiguration();
  if (!configuration.ok) {
    return configuration;
  }
  if (configuration.value === null) {
    return success(undefined);
  }
  const pending = await dependencies.loadPending();
  const home = await dependencies.loadHome();
  if (!pending.ok) {
    return pending;
  }
  if (!home.ok) {
    return home;
  }
  if (pending.value === null || home.value === null || pending.value !== home.value.identifier) {
    return dependencies.savePending(null);
  }
  const command = await dependencies.turnOffLights(configuration.value);
  if (!command.ok) {
    return command;
  }
  return dependencies.savePending(null);
}
