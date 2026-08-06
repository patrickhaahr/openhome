import { describe, expect, it } from 'vitest';

import type { Configuration } from '../domain/configuration';
import type { HomeGeofence } from '../domain/home-geofence';
import { success } from '../domain/result';
import { handleHomeGeofenceEvent, retryPendingHomeExit } from './handle-home-geofence-event';

const configuration = { baseUrl: 'http://openhome.local:8000', apiKey: 'secret' };
const home: HomeGeofence = { identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150, provider: 'expo' };

function dependencies(commands: Array<Configuration>, pending: Array<string | null>) {
  return {
    loadConfiguration: async () => success(configuration),
    loadHome: async () => success<HomeGeofence | null>(home),
    loadPending: async () => success(pending.at(-1) ?? null),
    savePending: async (value: string | null) => {
      pending.push(value);
      return success(undefined);
    },
    turnOffLights: async (value: Configuration) => {
      commands.push(value);
      return success(undefined);
    },
  };
}

describe('handleHomeGeofenceEvent', () => {
  it('turns off the lights when the device exits home', async () => {
    const commands: Array<Configuration> = [];
    const pending: Array<string | null> = [];

    const result = await handleHomeGeofenceEvent(
      { type: 'exit', regionIdentifier: home.identifier, regionState: 'outside' },
      dependencies(commands, pending),
    );

    expect(result).toEqual({ ok: true, value: undefined });
    expect(commands).toEqual([configuration]);
    expect(pending).toEqual([home.identifier, null]);
  });

  it('ignores unknown and stale exit events', async () => {
    const commands: Array<Configuration> = [];
    const pending: Array<string | null> = [];

    const unknown = await handleHomeGeofenceEvent(
      { type: 'exit', regionIdentifier: home.identifier, regionState: 'unknown' },
      dependencies(commands, pending),
    );
    const stale = await handleHomeGeofenceEvent(
      { type: 'exit', regionIdentifier: 'home-old', regionState: 'outside' },
      dependencies(commands, pending),
    );

    expect(unknown).toEqual({ ok: true, value: undefined });
    expect(stale).toEqual({ ok: true, value: undefined });
    expect(commands).toEqual([]);
    expect(pending).toEqual([]);
  });

  it('retries a pending light-off command', async () => {
    const commands: Array<Configuration> = [];
    const pending = [home.identifier];

    const result = await retryPendingHomeExit(dependencies(commands, pending));

    expect(result).toEqual({ ok: true, value: undefined });
    expect(commands).toEqual([configuration]);
    expect(pending).toEqual([home.identifier, null]);
  });

  it('discards a pending command for a replaced home geofence', async () => {
    const commands: Array<Configuration> = [];
    const pending = ['home-old'];

    const result = await retryPendingHomeExit(dependencies(commands, pending));

    expect(result).toEqual({ ok: true, value: undefined });
    expect(commands).toEqual([]);
    expect(pending).toEqual(['home-old', null]);
  });
});
