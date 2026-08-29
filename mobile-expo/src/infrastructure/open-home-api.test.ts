import { afterEach, describe, expect, it, vi } from "vitest";

import type { DockerContainer } from "../domain/docker";
import { failure, type Result } from "../domain/result";
import { createOpenHomeApi, parseAdguardStatus, parseIrStatus } from "./open-home-api";

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

describe("docker adapter", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  const configuration = { baseUrl: "http://openhome.test", apiKey: "secret" };
  const readError = "Couldn't read Docker containers from the Axum API.";
  const reachError = "Couldn't reach the Axum API. Check the Base URL and try again.";

  /** Deliberately loose so tests can also send malformed list payloads. */
  type ListPayload = { containers?: unknown; timestamp?: string };

  const pascalPayload = {
    containers: [
      {
        name: "adguard",
        status: "Up 3 days (healthy)",
        state: "running",
        HealthStatus: "healthy",
        uptime_seconds: "259200",
        image: "adguard/adguardhome",
        ports: ["53:53/tcp", "3000:3000/tcp"],
        labels: {},
        Created: "2026-08-26T10:00:00.000Z",
        restart_count: 2,
      },
    ],
    timestamp: "2026-08-29T12:00:00.000Z",
  };

  const snakePayload = {
    containers: [
      {
        name: "feed-rs",
        status: "Exited (0)",
        state: "exited",
        health_status: null,
        uptime_seconds: 90,
        image: "feed-rs:latest",
        ports: [],
        labels: {},
        created_at: "2026-08-01T10:00:00.000Z",
        restart_count: 0,
      },
    ],
    timestamp: "2026-08-29T12:00:00.000Z",
  };

  function stubFetch(handler: (init?: RequestInit) => Promise<Response>): void {
    vi.stubGlobal(
      "fetch",
      (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        void url;
        return handler(init);
      },
    );
  }

  function okFetch(body: ListPayload): void {
    stubFetch(async () => new Response(JSON.stringify(body), { status: 200 }));
  }

  async function listContainers(): Promise<Result<readonly DockerContainer[]>> {
    return createOpenHomeApi(configuration).docker.listContainers();
  }

  it("parses a PascalCase payload and normalizes string uptime", async () => {
    okFetch(pascalPayload);

    const result = await listContainers();

    expect(result).toEqual({
      ok: true,
      value: [
        {
          name: "adguard",
          state: "running",
          health: "healthy",
          image: "adguard/adguardhome",
          uptimeSeconds: 259_200,
          ports: ["53:53/tcp", "3000:3000/tcp"],
          restartCount: 2,
        },
      ],
    });
  });

  it("parses a snake_case payload with numeric uptime and null health", async () => {
    okFetch(snakePayload);

    const result = await listContainers();

    expect(result).toEqual({
      ok: true,
      value: [
        {
          name: "feed-rs",
          state: "exited",
          health: null,
          image: "feed-rs:latest",
          uptimeSeconds: 90,
          ports: [],
          restartCount: 0,
        },
      ],
    });
  });

  it("treats an unparseable string uptime as unknown", async () => {
    okFetch({
      containers: [{ ...pascalPayload.containers[0], uptime_seconds: "soon" }],
      timestamp: "t",
    });

    const result = await listContainers();

    expect(result.ok && result.value[0]?.uptimeSeconds).toBeNull();
  });

  it("rejects malformed payloads as failures", async () => {
    okFetch({ timestamp: "t" });
    expect(await listContainers()).toEqual(failure(readError));

    okFetch({ containers: "adguard", timestamp: "t" });
    expect(await listContainers()).toEqual(failure(readError));

    okFetch({ containers: ["adguard"], timestamp: "t" });
    expect(await listContainers()).toEqual(failure(readError));

    okFetch({ containers: [{ state: "running" }], timestamp: "t" });
    expect(await listContainers()).toEqual(failure(readError));
  });

  it("surfaces API error responses as failures", async () => {
    stubFetch(async () => new Response(JSON.stringify({ error: "Docker unavailable" }), { status: 503 }));

    const result = await listContainers();

    expect(result).toEqual(failure("Docker unavailable"));
  });

  it("uses the extended timeout for docker requests", async () => {
    vi.useFakeTimers();
    stubFetch(
      (init) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => reject(new Error("Aborted")));
        }),
    );

    let result: Result<readonly DockerContainer[]> | null = null;
    const pending = listContainers().then((value) => {
      result = value;
    });
    await vi.advanceTimersByTimeAsync(5_000);
    expect(result).toBeNull();

    await vi.advanceTimersByTimeAsync(10_000);
    await pending;
    expect(result).toEqual(failure(reachError));
  });

  it("keeps the short timeout for other adapter calls", async () => {
    vi.useFakeTimers();
    stubFetch(
      (init) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => reject(new Error("Aborted")));
        }),
    );

    let result: Result<void> | null = null;
    const pending = createOpenHomeApi(configuration)
      .validateConfiguration()
      .then((value) => {
        result = value;
      });
    await vi.advanceTimersByTimeAsync(5_000);
    await pending;
    expect(result).toEqual(failure(reachError));
  });
});
