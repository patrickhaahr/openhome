import { describe, expect, it } from "vitest";

import { formatPauseRemaining, protectionPhase, type AdguardStatus } from "./adguard";

const status = (overrides: Partial<AdguardStatus> = {}): AdguardStatus => ({
  version: "v0.107.52",
  running: true,
  protectionEnabled: true,
  pauseEndsAtMs: null,
  ...overrides,
});

describe("protectionPhase", () => {
  const now = 1_000_000;

  it("reports protected while protection is enabled", () => {
    expect(protectionPhase(status(), now)).toBe("protected");
  });

  it("reports paused while a pause has not ended", () => {
    expect(protectionPhase(status({ protectionEnabled: false, pauseEndsAtMs: now + 1 }), now)).toBe(
      "paused",
    );
  });

  it("reports unprotected when protection is off without a pause", () => {
    expect(protectionPhase(status({ protectionEnabled: false }), now)).toBe("unprotected");
  });

  it("reports unprotected once a pause has expired", () => {
    expect(protectionPhase(status({ protectionEnabled: false, pauseEndsAtMs: now }), now)).toBe(
      "unprotected",
    );
  });
});

describe("formatPauseRemaining", () => {
  it("formats minutes and seconds", () => {
    expect(formatPauseRemaining(59 * 60_000 + 12_000)).toBe("59m 12s");
    expect(formatPauseRemaining(45_000)).toBe("45s");
  });

  it("formats hours without seconds", () => {
    expect(formatPauseRemaining(3_661_000)).toBe("1h 1m");
  });

  it("clamps negative remainings to zero", () => {
    expect(formatPauseRemaining(-5_000)).toBe("0s");
  });
});
