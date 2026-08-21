import { isJsonNumber, isJsonObject, isJsonString, type Json } from "./json";
import { failure, success, type Result } from "./result";

/** A home region monitored by the operating system. */
export type HomeGeofence = {
  readonly identifier: string;
  readonly latitude: number;
  readonly longitude: number;
  readonly radiusMeters: number;
  readonly provider: HomeGeofenceProvider;
};

/** Location backend used to monitor the Home Geofence. */
export type HomeGeofenceProvider = "expo" | "native";

/** Parse a user-entered geofence radius in meters. */
export function parseRadiusMeters(input: string): Result<number> {
  const value = Number(input.trim());
  if (!Number.isFinite(value) || !Number.isInteger(value)) {
    return failure("Enter a whole number of meters.");
  }
  if (value < 100) {
    return failure("Enter a radius of at least 100 meters.");
  }
  if (value > 10_000) {
    return failure("Enter a radius no greater than 10,000 meters.");
  }
  return success(value);
}

/** Parse persisted data into a valid home geofence. */
export function parseHomeGeofence(json: Json): Result<HomeGeofence> {
  if (!isJsonObject(json)) {
    return invalidHomeGeofence();
  }
  const identifier = json["identifier"];
  const latitude = json["latitude"];
  const longitude = json["longitude"];
  const radiusMeters = json["radiusMeters"];
  if (!isJsonString(identifier) || identifier.length === 0) {
    return invalidHomeGeofence();
  }
  if (!isJsonNumber(latitude) || latitude < -90 || latitude > 90) {
    return invalidHomeGeofence();
  }
  if (!isJsonNumber(longitude) || longitude < -180 || longitude > 180) {
    return invalidHomeGeofence();
  }
  if (
    !isJsonNumber(radiusMeters) ||
    !Number.isInteger(radiusMeters) ||
    radiusMeters < 100 ||
    radiusMeters > 10_000
  ) {
    return invalidHomeGeofence();
  }
  const provider = json["provider"] ?? "expo";
  if (!isHomeGeofenceProvider(provider)) {
    return invalidHomeGeofence();
  }
  return success({ identifier, latitude, longitude, radiusMeters, provider });
}

function isHomeGeofenceProvider(value: Json): value is HomeGeofenceProvider {
  return value === "expo" || value === "native";
}

function invalidHomeGeofence(): Result<never> {
  return failure("The saved home location could not be read. Set it again.");
}
