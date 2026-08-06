# ADR 0001: Native Android Home Geofence Backend

## Status

Accepted

## Context

The existing `expo-location` backend uses Google Play Services for Android geofencing. It cannot register a Home Geofence in a GrapheneOS profile where sandboxed Google Play is not installed, although Android framework location providers remain available.

The existing infrastructure modules were audited before adding another adapter. `HomeGeofenceService` already owns persistence and replacement semantics, while `home-geofence-task.ts` owns delivery to the Axum API. Extending either with Android framework calls would couple portable application behavior to one platform and would not provide Expo Modules autolinking or process restart support.

## Decision

Keep `HomeGeofenceService` as the provider-selecting module and provide two infrastructure adapters behind its existing seam:

- Expo Location remains available as the `expo` provider.
- A local Android Expo module implements the `native` provider with AOSP `LocationManager` APIs.

The native module uses `getCurrentLocation` for the home center and `addProximityAlert` for background exit monitoring without continuous location updates. Its Expo TaskManager consumer translates exits into the existing background JavaScript task payload. TaskManager persists registration and restores it after process death, reboot, and package replacement. Android force-stop suppresses broadcasts until the user opens the app again; the app does not attempt to bypass that platform behavior.

Provider choice is explicit in the UI and persisted with the Home Geofence. Existing persisted geofences without a provider remain on Expo.

## Consequences

The native path does not require Google Play Services and does not duplicate API credentials or Axum calls in Kotlin. It is Android-only and requires a development or release build rather than Expo Go. Both providers continue to share one application flow and one exit-command handler.
