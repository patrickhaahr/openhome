/** Anonymous labels for the demo store's existing household owners. */
export const demoMembers = [
  { id: "Alex", label: "User 1", icon: "1" },
  { id: "Riley", label: "User 2", icon: "2" },
] as const;

/** Ownership belongs to list entries and writes, never the dashboard session. */
export type DemoMemberId = (typeof demoMembers)[number]["id"];
