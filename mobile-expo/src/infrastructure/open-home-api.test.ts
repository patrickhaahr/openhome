import { describe, expect, it } from "vitest";

import { failure } from "../domain/result";
import { parseAdguardStatus, parseIrStatus } from "./open-home-api";

describe("parseIrStatus", () => {
  it("parses and deduplicates each remote command set", () => {
    const result = parseIrStatus({
      message: "IR ready",
      remotes: {
        edifier: ["bluetooth", "bluetooth", " optical "],
        lgtv: ["power"],
      },
    });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.message).toBe("IR ready");
      expect([...result.value.edifierCommands]).toEqual(["bluetooth", "optical"]);
      expect([...result.value.lgTvCommands]).toEqual(["power"]);
    }
  });

  it("rejects malformed remote command lists", () => {
    expect(parseIrStatus({ remotes: { edifier: ["power"], lgtv: "power" } })).toEqual({
      ok: false,
      error: "Couldn't read IR status from the Axum API.",
    });
  });
});

describe("parseAdguardStatus", () => {
  const now = 1_000_000_000;
  const readError = "Couldn't read AdGuard status from the Axum API.";

  it("parses a protected status with version and running indicator", () => {
    const result = parseAdguardStatus(
      {
        protection_enabled: true,
        protection_disabled_duration: 0,
        protection_disabled_until: null,
        version: "v0.107.52",
        running: true,
      },
      now,
    );

    expect(result).toEqual({
      ok: true,
      value: {
        version: "v0.107.52",
        running: true,
        protectionEnabled: true,
        pauseEndsAtMs: null,
      },
    });
  });

  it("takes the pause end from the until-timestamp", () => {
    const result = parseAdguardStatus(
      {
        protection_enabled: false,
        protection_disabled_duration: 300_000,
        protection_disabled_until: "2026-08-21T10:30:00.000Z",
        version: "v0.107.52",
        running: true,
      },
      now,
    );

    expect(result.ok && result.value.pauseEndsAtMs).toBe(Date.parse("2026-08-21T10:30:00.000Z"));
  });

  it("falls back to now plus duration when no until-timestamp is present", () => {
    const result = parseAdguardStatus(
      {
        protection_enabled: false,
        protection_disabled_duration: 300_000,
        protection_disabled_until: null,
        version: "v0.107.52",
        running: true,
      },
      now,
    );

    expect(result.ok && result.value.pauseEndsAtMs).toBe(now + 300_000);
  });

  it("rejects malformed payloads as failures", () => {
    expect(parseAdguardStatus(null, now)).toEqual({ ok: false, error: readError });
    expect(
      parseAdguardStatus({ protection_enabled: "yes", version: "v1", running: true }, now),
    ).toEqual(failure(readError));
    expect(parseAdguardStatus({ protection_enabled: true, version: "v1" }, now)).toEqual(
      failure(readError),
    );
    expect(
      parseAdguardStatus({ protection_enabled: true, version: 3, running: true }, now),
    ).toEqual(failure(readError));
  });
});
