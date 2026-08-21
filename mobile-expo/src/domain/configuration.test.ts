import { describe, expect, it } from "vitest";

import { parseConfiguration } from "./configuration";

describe("parseConfiguration", () => {
  it("normalizes quoted input and a trailing slash", () => {
    expect(parseConfiguration(' "http://openhome.local:8000/" ', ' "secret" ')).toEqual({
      ok: true,
      value: { baseUrl: "http://openhome.local:8000", apiKey: "secret" },
    });
  });

  it("rejects a query before configuration reaches storage", () => {
    expect(parseConfiguration("http://openhome.local:8000?debug=true", "secret")).toEqual({
      ok: false,
      error: "Base URL must not include a query or fragment.",
    });
  });

  it("requires an API key", () => {
    expect(parseConfiguration("http://openhome.local:8000", " ")).toEqual({
      ok: false,
      error: "Enter an API Key.",
    });
  });
});
