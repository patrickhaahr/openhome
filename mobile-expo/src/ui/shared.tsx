import { ActivityIndicator, Pressable, StyleSheet, Text, View } from "react-native";

import { colors } from "./theme";

/** Render a titled page heading shared by the top-level pages. */
export function PageHeading({
  eyebrow,
  title,
  description,
}: {
  readonly eyebrow: string;
  readonly title: string;
  readonly description: string;
}) {
  return (
    <View>
      <Text style={styles.pageEyebrow}>{eyebrow}</Text>
      <Text accessibilityRole="header" style={styles.pageTitle}>
        {title}
      </Text>
      <Text style={styles.pageDescription}>{description}</Text>
    </View>
  );
}

/** Render a filled primary action shared by the top-level pages. */
export function ActionButton({
  label,
  sending,
  disabled,
  onPress,
}: {
  readonly label: string;
  readonly sending: boolean;
  readonly disabled: boolean;
  readonly onPress: () => void;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled, busy: sending }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.actionButton,
        disabled && styles.disabled,
        pressed && styles.actionPressed,
      ]}
    >
      {sending ? <ActivityIndicator color={colors.background} size="small" /> : null}
      <Text style={styles.actionLabel}>{label}</Text>
    </Pressable>
  );
}

/** Render an outlined secondary action shared by the top-level pages. */
export function SecondaryAction({
  label,
  disabled,
  onPress,
}: {
  readonly label: string;
  readonly disabled: boolean;
  readonly onPress: () => void;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.secondaryAction,
        disabled && styles.disabled,
        pressed && styles.iconPressed,
      ]}
    >
      <Text style={styles.secondaryActionLabel}>{label}</Text>
    </Pressable>
  );
}

/** Layout and action styles shared across top-level pages. */
export const styles = StyleSheet.create({
  actionButton: {
    alignItems: "center",
    backgroundColor: colors.signal,
    borderRadius: 14,
    flex: 1,
    flexDirection: "row",
    gap: 8,
    justifyContent: "center",
    minHeight: 54,
    paddingHorizontal: 10,
  },
  actionLabel: {
    color: colors.background,
    fontSize: 14,
    fontWeight: "800",
    lineHeight: 19,
    textAlign: "center",
  },
  actionPressed: { opacity: 0.84, transform: [{ scale: 0.96 }] },
  compactAction: {
    alignItems: "center",
    backgroundColor: colors.panelRaised,
    borderColor: colors.border,
    borderRadius: 12,
    borderWidth: 1,
    flex: 1,
    justifyContent: "center",
    minHeight: 48,
    paddingHorizontal: 8,
  },
  compactActionLabel: { color: colors.text, fontSize: 14, fontWeight: "700" },
  disabled: { opacity: 0.48 },
  error: { color: colors.danger, fontSize: 13, lineHeight: 19, textAlign: "center" },
  iconPressed: { opacity: 0.72 },
  pageContent: { padding: 20, paddingBottom: 32 },
  pageDescription: { color: colors.muted, fontSize: 16, lineHeight: 23, marginTop: 5 },
  pageEyebrow: { color: colors.signal, fontSize: 11, fontWeight: "800", letterSpacing: 1.6 },
  pageTitle: {
    color: colors.text,
    fontSize: 40,
    fontWeight: "900",
    letterSpacing: -1.6,
    lineHeight: 44,
    marginTop: 3,
  },
  retry: { color: colors.signal, fontSize: 14, fontWeight: "800" },
  retryButton: { minHeight: 44, justifyContent: "center", paddingHorizontal: 12 },
  row: { flexDirection: "row", gap: 10 },
  secondaryAction: {
    alignItems: "center",
    borderColor: colors.border,
    borderRadius: 14,
    borderWidth: 1,
    flex: 1,
    justifyContent: "center",
    minHeight: 54,
    paddingHorizontal: 10,
  },
  secondaryActionLabel: {
    color: colors.text,
    fontSize: 14,
    fontWeight: "700",
    lineHeight: 19,
    textAlign: "center",
  },
  section: {
    backgroundColor: colors.panel,
    borderColor: colors.border,
    borderRadius: 20,
    borderWidth: 1,
    gap: 14,
    padding: 16,
  },
  sectionDetail: { color: colors.muted, fontSize: 12, fontWeight: "600" },
  sectionHeader: { alignItems: "center", flexDirection: "row", justifyContent: "space-between" },
  sectionTitle: { color: colors.muted, fontSize: 11, fontWeight: "800", letterSpacing: 1.4 },
  stack: { gap: 18, width: "100%" },
  statusPanel: {
    alignItems: "center",
    backgroundColor: colors.panel,
    borderColor: colors.border,
    borderRadius: 16,
    borderWidth: 1,
    flexDirection: "row",
    gap: 9,
    justifyContent: "center",
    minHeight: 52,
    paddingHorizontal: 14,
    paddingVertical: 10,
  },
  statusText: { color: colors.text, fontSize: 14, fontWeight: "700" },
});
