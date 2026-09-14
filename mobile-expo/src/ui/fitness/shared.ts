import { StyleSheet } from "react-native";

import type { Exercise } from "../../domain/fitness";
import { colors } from "../theme";

export const styles = StyleSheet.create({
  card: { gap: 14 },
  chip: {
    alignItems: "center",
    backgroundColor: colors.panelRaised,
    borderColor: colors.border,
    borderRadius: 12,
    borderWidth: 1,
    flex: 1,
    justifyContent: "center",
    minHeight: 44,
    paddingHorizontal: 6,
  },
  chipLabel: { color: colors.muted, fontSize: 13, fontWeight: "700" },
  chipLabelSelected: { color: colors.signal },
  chipSelected: { backgroundColor: colors.signalDark, borderColor: colors.signal },
  chipWrap: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  pickChip: {
    alignItems: "center",
    backgroundColor: colors.panelRaised,
    borderColor: colors.border,
    borderRadius: 12,
    borderWidth: 1,
    justifyContent: "center",
    minHeight: 44,
    paddingHorizontal: 16,
  },
  detail: { color: colors.muted, fontSize: 13, lineHeight: 19 },
  formHeader: { minHeight: 24, flexDirection: "row", justifyContent: "space-between" },
  formToggle: { color: colors.signal, fontSize: 12, fontWeight: "800" },
  groupLabel: { color: colors.signal, fontSize: 12, fontWeight: "800", letterSpacing: 1.2 },
  halfInput: { flex: 1 },
  input: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: 12,
    borderWidth: 1,
    color: colors.text,
    fontSize: 16,
    minHeight: 50,
    paddingHorizontal: 14,
    paddingVertical: 12,
  },
  name: { color: colors.text, fontSize: 17, fontWeight: "800", letterSpacing: -0.2 },
  rowAction: { justifyContent: "center", minHeight: 32 },
  rowActionConfirm: { color: colors.danger },
  rowActionLabel: { color: colors.signal, fontSize: 13, fontWeight: "700" },
  rowActions: { flexDirection: "row", gap: 18 },
  rowChevron: { color: colors.signal, fontSize: 12, fontWeight: "800" },
  rowHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    minHeight: 24,
  },
});

/** Today's date as the log form's YYYY-MM-DD default. */
export function today(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function formatDate(date: string): string {
  const parsed = new Date(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) {
    return date;
  }
  return parsed.toLocaleDateString(undefined, {
    weekday: "short",
    year: "numeric",
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  });
}

/** Group exercises by muscle group for pickers; blank groups land in "Other". */
export function groupByMuscleGroup(
  exercises: readonly Exercise[],
): ReadonlyArray<[string, Exercise[]]> {
  const groups = new Map<string, Exercise[]>();
  for (const exercise of exercises) {
    const key = exercise.muscleGroup?.trim() || "Other";
    const existing = groups.get(key);
    if (existing === undefined) {
      groups.set(key, [exercise]);
    } else {
      existing.push(exercise);
    }
  }
  return [...groups.entries()];
}
