/** Public Copenhagen demonstration coordinates, unrelated to household configuration. */
export const DEMO_CENTER: [number, number] = [12.5692, 55.6869];

/** Illustrative geometry, not the output of a routing engine. */
export const DEMO_ROUTE = {
  type: "Feature" as const,
  properties: {},
  geometry: {
    type: "LineString" as const,
    coordinates: [
      [12.5682, 55.6837],
      [12.5658, 55.6879],
      [12.5608, 55.6915],
      [12.5648, 55.6935],
      [12.5706, 55.6956],
      [12.5765, 55.6937],
      [12.5796, 55.6905],
      [12.5758, 55.6881],
      [12.5728, 55.6867],
      [12.5682, 55.6837],
    ],
  },
};
