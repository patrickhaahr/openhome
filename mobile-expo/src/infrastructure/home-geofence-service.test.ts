import { describe, expect, it } from "vitest";

import type { HomeGeofence, HomeGeofenceProvider } from "../domain/home-geofence";
import { failure, success } from "../domain/result";
import { createHomeGeofenceService, type HomeGeofenceBackend } from "./home-geofence-service";
import type { HomeGeofenceStore } from "./home-geofence-store";

const previous: HomeGeofence = {
  identifier: "home-old",
  latitude: 51.5074,
  longitude: -0.1278,
  radiusMeters: 150,
  provider: "expo",
};

function backend(provider: HomeGeofenceProvider, calls: Array<string>): HomeGeofenceBackend {
  return {
    checkAvailability: async () => success(undefined),
    getCurrentPosition: async () =>
      success({ latitude: 51.5074, longitude: -0.1278, accuracyMeters: 12 }),
    start: async (home) => {
      calls.push(`start:${provider}:${home.identifier}`);
    },
    stop: async () => {
      calls.push(`stop:${provider}`);
    },
  };
}

function store(initial: HomeGeofence | null, saved: Array<HomeGeofence | null>): HomeGeofenceStore {
  let home = initial;
  return {
    load: async () => success(home),
    save: async (value) => {
      home = value;
      saved.push(value);
      return success(undefined);
    },
    loadPending: async () => success(null),
    savePending: async () => success(undefined),
  };
}

function homeStore(home: HomeGeofence | null, save: HomeGeofenceStore["save"]): HomeGeofenceStore {
  return {
    load: async () => success(home),
    save,
    loadPending: async () => success(null),
    savePending: async () => success(undefined),
  };
}

describe("createHomeGeofenceService", () => {
  it("sets and persists home with the explicitly selected native provider", async () => {
    const calls: Array<string> = [];
    const saved: Array<HomeGeofence | null> = [];
    const service = createHomeGeofenceService(store(null, saved), {
      expo: backend("expo", calls),
      native: backend("native", calls),
    });

    const result = await service.setAtCurrentLocation(150, "native");

    expect(result.ok && result.value.provider).toBe("native");
    expect(saved).toHaveLength(1);
    expect(saved[0]?.provider).toBe("native");
    expect(calls).toEqual([`start:native:${saved[0]?.identifier}`]);
  });

  it("stops the previous provider after switching providers", async () => {
    const calls: Array<string> = [];
    const service = createHomeGeofenceService(store(previous, []), {
      expo: backend("expo", calls),
      native: backend("native", calls),
    });

    const result = await service.setAtCurrentLocation(200, "native");

    expect(result.ok).toBe(true);
    expect(calls[0]?.startsWith("start:native:")).toBe(true);
    expect(calls[1]).toBe("stop:expo");
  });

  it("stops the attempted provider before rollback persistence can fail", async () => {
    const calls: Array<string> = [];
    let saves = 0;
    const expo = backend("expo", calls);
    const service = createHomeGeofenceService(
      homeStore(previous, async () => {
        saves += 1;
        return saves === 1 ? success(undefined) : failure("save failed");
      }),
      {
        expo: {
          ...expo,
          stop: async () => {
            calls.push("stop:expo");
            throw new Error("stop failed");
          },
        },
        native: backend("native", calls),
      },
    );

    const result = await service.setAtCurrentLocation(200, "native");

    expect(result.ok).toBe(false);
    expect(calls[0]?.startsWith("start:native:")).toBe(true);
    expect(calls.slice(1)).toEqual(["stop:expo", "stop:native"]);
  });

  it("does not report disabled when monitoring could not be stopped", async () => {
    const saved: Array<HomeGeofence | null> = [];
    const expo = backend("expo", []);
    const service = createHomeGeofenceService(store(previous, saved), {
      expo: {
        ...expo,
        stop: async () => {
          throw new Error("stop failed");
        },
      },
      native: backend("native", []),
    });

    const result = await service.disable();

    expect(result).toEqual({ ok: false, error: "Couldn't stop monitoring the home location." });
    expect(saved).toEqual([]);
  });

  it("restores monitoring when persisting disabled state fails", async () => {
    const calls: Array<string> = [];
    const service = createHomeGeofenceService(
      homeStore(previous, async () => failure("save failed")),
      {
        expo: backend("expo", calls),
        native: backend("native", calls),
      },
    );

    const result = await service.disable();

    expect(result).toEqual({ ok: false, error: "save failed" });
    expect(calls).toEqual(["stop:expo", `start:expo:${previous.identifier}`]);
  });

  it("resumes monitoring the persisted provider when the app becomes active", async () => {
    const calls: Array<string> = [];
    const service = createHomeGeofenceService(store({ ...previous, provider: "native" }, []), {
      expo: backend("expo", calls),
      native: backend("native", calls),
    });

    const result = await service.resume();

    expect(result).toEqual({ ok: true, value: undefined });
    expect(calls).toEqual([`start:native:${previous.identifier}`]);
  });
});
