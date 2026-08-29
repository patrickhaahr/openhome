import { describe, expect, it } from "vitest";

import {
  classificationCounts,
  classifyContainer,
  dockerHealthSummary,
  formatPorts,
  formatUptime,
  type DockerContainer,
} from "./docker";

function container(overrides: Partial<DockerContainer>): DockerContainer {
  return {
    name: "adguard",
    state: "running",
    health: null,
    image: "adguard/adguardhome",
    uptimeSeconds: 3661,
    ports: [],
    restartCount: 0,
    ...overrides,
  };
}

describe("classifyContainer", () => {
  it("classifies running containers by health verdict", () => {
    expect(classifyContainer(container({}))).toBe("healthy");
    expect(classifyContainer(container({ health: "healthy" }))).toBe("healthy");
    expect(classifyContainer(container({ health: "unhealthy" }))).toBe("unhealthy");
  });

  it("classifies containers that are not running as stopped", () => {
    expect(classifyContainer(container({ state: "exited" }))).toBe("stopped");
    expect(classifyContainer(container({ state: "Exited", health: "unhealthy" }))).toBe("stopped");
  });
});

describe("classificationCounts", () => {
  it("counts each classification alongside the total", () => {
    const counts = classificationCounts([
      container({ name: "a" }),
      container({ name: "b", health: "unhealthy" }),
      container({ name: "c", state: "exited" }),
      container({ name: "d", state: "exited" }),
    ]);

    expect(counts).toEqual({ all: 4, healthy: 1, unhealthy: 1, stopped: 2 });
  });
});

describe("dockerHealthSummary", () => {
  it("is idle when nothing is running", () => {
    expect(dockerHealthSummary([])).toBe("idle");
    expect(dockerHealthSummary([container({ state: "exited" })])).toBe("idle");
  });

  it("is unhealthy when a running container is unhealthy", () => {
    const containers = [container({ name: "a" }), container({ name: "b", health: "unhealthy" })];
    expect(dockerHealthSummary(containers)).toBe("unhealthy");
  });

  it("is healthy when running containers have no unhealthy verdict", () => {
    expect(dockerHealthSummary([container({ name: "a" })])).toBe("healthy");
  });
});

describe("formatUptime", () => {
  it("renders compact uptime from seconds", () => {
    expect(formatUptime(null)).toBe("Unknown uptime");
    expect(formatUptime(30)).toBe("30s");
    expect(formatUptime(90)).toBe("1m");
    expect(formatUptime(3661)).toBe("1h 1m");
    expect(formatUptime(90000)).toBe("1d 1h");
  });
});

describe("formatPorts", () => {
  it("joins port mappings or reports none", () => {
    expect(formatPorts(["53:53/tcp", "3000:3000/tcp"])).toBe("53:53/tcp, 3000:3000/tcp");
    expect(formatPorts([])).toBe("No ports exposed");
  });
});
