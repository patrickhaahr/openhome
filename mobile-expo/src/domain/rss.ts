/** One compact Feed item in the timeline, as served by the Axum API. */
export type TimelineItem = {
  readonly id: number;
  readonly title: string;
  readonly description: string | null;
  readonly link: string;
};
