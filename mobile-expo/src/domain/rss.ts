import { failure, success, type Result } from "./result";

/** One subscribed Feed, as served by the Axum API. */
export type Feed = {
  readonly id: number;
  readonly url: string;
  /** The feed's title once fetched; null until the first refresh. */
  readonly title: string | null;
};

/** One compact Feed item in the timeline, as served by the Axum API. */
export type TimelineItem = {
  readonly id: number;
  readonly title: string;
  readonly description: string | null;
  readonly link: string;
};

const INVALID_FEED_URL = "Enter a valid feed URL starting with http:// or https://.";

/** Validate a pasted Feed URL on the client before any request is sent. */
export function parseFeedUrl(input: string): Result<string> {
  const trimmed = input.trim();
  if (trimmed.length === 0) {
    return failure("Enter a feed URL to add.");
  }
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return failure(INVALID_FEED_URL);
  }
  if ((url.protocol !== "http:" && url.protocol !== "https:") || url.host === "") {
    return failure(INVALID_FEED_URL);
  }
  return success(trimmed);
}
