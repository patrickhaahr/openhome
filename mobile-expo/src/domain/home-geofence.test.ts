import { describe, expect, it } from 'vitest';

import { parseHomeGeofence, parseRadiusMeters } from './home-geofence';

describe('parseRadiusMeters', () => {
  it('parses a whole number of meters', () => {
    expect(parseRadiusMeters('250')).toEqual({ ok: true, value: 250 });
  });

  it('rejects unreliable and fractional distances', () => {
    expect(parseRadiusMeters('99')).toEqual({ ok: false, error: 'Enter a radius of at least 100 meters.' });
    expect(parseRadiusMeters('10.5')).toEqual({ ok: false, error: 'Enter a whole number of meters.' });
  });
});

describe('parseHomeGeofence', () => {
  it('parses a stored home geofence', () => {
    expect(parseHomeGeofence({ identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150, provider: 'native' })).toEqual({
      ok: true,
      value: { identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150, provider: 'native' },
    });
  });

  it('keeps existing saved geofences on the Expo provider', () => {
    expect(parseHomeGeofence({ identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150 })).toEqual({
      ok: true,
      value: { identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150, provider: 'expo' },
    });
  });

  it('rejects coordinates outside the earth bounds', () => {
    expect(parseHomeGeofence({ identifier: 'home-123', latitude: 91, longitude: -0.1278, radiusMeters: 150 })).toEqual({
      ok: false,
      error: 'The saved home location could not be read. Set it again.',
    });
  });

  it('rejects an unknown location provider', () => {
    expect(parseHomeGeofence({ identifier: 'home-123', latitude: 51.5074, longitude: -0.1278, radiusMeters: 150, provider: 'other' })).toEqual({
      ok: false,
      error: 'The saved home location could not be read. Set it again.',
    });
  });
});
