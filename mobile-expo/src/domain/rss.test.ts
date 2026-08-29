import { describe, expect, it } from "vitest";

import { failure } from "./result";
import { parseFeedUrl } from "./rss";

describe("parseFeedUrl", () => {
  it("accepts absolute http(s) URLs and returns them trimmed", () => {
    expect(parseFeedUrl("  https://blog.test/feed.xml  ")).toEqual({
      ok: true,
      value: "https://blog.test/feed.xml",
    });
    expect(parseFeedUrl("http://blog.test/feed.xml")).toEqual({
      ok: true,
      value: "http://blog.test/feed.xml",
    });
  });

  it("rejects blank input before any request is sent", () => {
    expect(parseFeedUrl("   ")).toEqual(failure("Enter a feed URL to add."));
  });

  it("rejects input without a usable URL scheme", () => {
    const invalid = "Enter a valid feed URL starting with http:// or https://.";
    expect(parseFeedUrl("not a url")).toEqual(failure(invalid));
    expect(parseFeedUrl("blog.test/feed.xml")).toEqual(failure(invalid));
    expect(parseFeedUrl("ftp://blog.test/feed.xml")).toEqual(failure(invalid));
  });
});
