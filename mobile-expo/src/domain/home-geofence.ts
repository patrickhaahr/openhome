import { failure, success, type Result } from './result';

/** A home region monitored by the operating system. */
export type HomeGeofence = {
  readonly identifier: string;
  readonly latitude: number;
  readonly longitude: number;
  readonly radiusMeters: number;
};

/** Parse a user-entered geofence radius in meters. */
export function parseRadiusMeters(input: string): Result<number> {
  const value = Number(input.trim());
  if (!Number.isFinite(value) || !Number.isInteger(value)) {
    return failure('Enter a whole number of meters.');
  }
  if (value < 100) {
    return failure('Enter a radius of at least 100 meters.');
  }
  if (value > 10_000) {
    return failure('Enter a radius no greater than 10,000 meters.');
  }
  return success(value);
}

/** Parse persisted data into a valid home geofence. */
export function parseHomeGeofence(value: unknown): Result<HomeGeofence> {
  if (typeof value !== 'object' || value === null) {
    return invalidHomeGeofence();
  }
  if (!('identifier' in value) || typeof value.identifier !== 'string' || value.identifier.length === 0) {
    return invalidHomeGeofence();
  }
  if (!('latitude' in value) || !isFiniteNumber(value.latitude) || value.latitude < -90 || value.latitude > 90) {
    return invalidHomeGeofence();
  }
  if (!('longitude' in value) || !isFiniteNumber(value.longitude) || value.longitude < -180 || value.longitude > 180) {
    return invalidHomeGeofence();
  }
  if (!('radiusMeters' in value) || !isFiniteNumber(value.radiusMeters) || !Number.isInteger(value.radiusMeters) || value.radiusMeters < 100 || value.radiusMeters > 10_000) {
    return invalidHomeGeofence();
  }
  return success({ identifier: value.identifier, latitude: value.latitude, longitude: value.longitude, radiusMeters: value.radiusMeters });
}

function invalidHomeGeofence(): Result<never> {
  return failure('The saved home location could not be read. Set it again.');
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}
