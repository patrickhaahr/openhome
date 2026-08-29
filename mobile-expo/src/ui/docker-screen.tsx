import {
  ActivityIndicator,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useState } from "react";

import type { ContainerAction, DockerActions, DockerState } from "../application/use-docker";
import {
  classifyContainer,
  formatPorts,
  formatUptime,
  type ClassificationCounts,
  type ContainerClassification,
  type DockerContainer,
} from "../domain/docker";
import { PageHeading, styles as shared } from "./shared";
import { colors } from "./theme";

type DockerFilter = "all" | ContainerClassification;

type Props = {
  readonly state: DockerState;
  readonly actions: DockerActions;
  readonly counts: ClassificationCounts;
};

const filters: ReadonlyArray<{ readonly key: DockerFilter; readonly label: string }> = [
  { key: "all", label: "All" },
  { key: "healthy", label: "Healthy" },
  { key: "unhealthy", label: "Unhealthy" },
  { key: "stopped", label: "Stopped" },
];

const classificationLabel = {
  healthy: "Healthy",
  unhealthy: "Unhealthy",
  stopped: "Stopped",
} satisfies Record<ContainerClassification, string>;

const classificationColor = {
  healthy: colors.ready,
  unhealthy: colors.danger,
  stopped: colors.muted,
} satisfies Record<ContainerClassification, string>;

/** Render the Docker Tab: container list filtered by health classification. */
export function DockerPage({ state, actions, counts }: Props) {
  const [filter, setFilter] = useState<DockerFilter>("all");
  const containers = state.tag === "ready" ? state.containers : [];
  const visible =
    filter === "all"
      ? containers
      : containers.filter((container) => classifyContainer(container) === filter);

  return (
    <ScrollView
      contentContainerStyle={shared.pageContent}
      refreshControl={
        <RefreshControl
          refreshing={state.tag === "ready" && state.refreshing}
          onRefresh={actions.refresh}
          colors={[colors.signal]}
          tintColor={colors.signal}
        />
      }
    >
      <View style={shared.stack}>
        <PageHeading
          eyebrow="SERVER OPERATIONS"
          title="Docker"
          description="Every container on the homelab server."
        />
        <View style={shared.row}>
          {filters.map(({ key, label }) => (
            <Pressable
              key={key}
              accessibilityLabel={`${label} containers: ${counts[key]}`}
              accessibilityRole="button"
              accessibilityState={{ selected: filter === key }}
              onPress={() => {
                setFilter(key);
              }}
              style={({ pressed }) => [
                styles.chip,
                filter === key && styles.chipSelected,
                pressed && shared.actionPressed,
              ]}
            >
              <Text style={[styles.chipLabel, filter === key && styles.chipLabelSelected]}>
                {label} {counts[key]}
              </Text>
            </Pressable>
          ))}
        </View>

        {state.tag === "loading" ? (
          <View style={shared.statusPanel}>
            <ActivityIndicator color={colors.signal} />
            <Text style={shared.statusText}>Loading containers</Text>
          </View>
        ) : null}
        {state.tag === "error" ? (
          <View style={shared.section}>
            <Text accessibilityRole="alert" style={shared.error}>
              {state.message}
            </Text>
            <Pressable
              accessibilityRole="button"
              onPress={actions.refresh}
              style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
            >
              <Text style={shared.retry}>Try again</Text>
            </Pressable>
          </View>
        ) : null}
        {state.tag === "ready" && visible.length === 0 ? (
          <View style={shared.statusPanel}>
            <Text style={shared.statusText}>
              {containers.length === 0 ? "No containers" : `No ${filter} containers`}
            </Text>
          </View>
        ) : null}
        {state.tag === "ready"
          ? visible.map((container) => (
              <ContainerRow
                key={container.name}
                container={container}
                acting={state.acting}
                actions={actions}
              />
            ))
          : null}
        {state.tag === "ready" && state.error !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.error}
          </Text>
        ) : null}
      </View>
    </ScrollView>
  );
}

function ContainerRow({
  container,
  acting,
  actions,
}: {
  readonly container: DockerContainer;
  readonly acting: { readonly name: string; readonly action: ContainerAction } | null;
  readonly actions: DockerActions;
}) {
  const classification = classifyContainer(container);
  return (
    <View style={shared.section}>
      <View style={shared.sectionHeader}>
        <View style={styles.identity}>
          <View style={[styles.dot, { backgroundColor: classificationColor[classification] }]} />
          <Text style={styles.name}>{container.name}</Text>
        </View>
        <Text style={shared.sectionDetail}>{classificationLabel[classification]}</Text>
      </View>
      <Text numberOfLines={1} style={styles.image}>
        {container.image}
      </Text>
      <Text style={styles.detail}>
        {capitalize(container.state)} · {formatUptime(container.uptimeSeconds)}
      </Text>
      <Text style={styles.detail}>{formatPorts(container.ports)}</Text>
      <Text style={styles.detail}>Restarts: {container.restartCount}</Text>
      <View style={shared.row}>
        {(
          [
            ["start", "Start"],
            ["stop", "Stop"],
            ["restart", "Restart"],
          ] as const
        ).map(([action, label]) => (
          <RowAction
            key={action}
            label={label}
            accessibilityLabel={`${label} container ${container.name}`}
            busy={acting?.name === container.name && acting.action === action}
            disabled={acting !== null}
            onPress={() => actions[`${action}Container`](container.name)}
          />
        ))}
      </View>
    </View>
  );
}

/** Render one immediate lifecycle action for a container row. */
function RowAction({
  label,
  accessibilityLabel,
  busy,
  disabled,
  onPress,
}: {
  readonly label: string;
  readonly accessibilityLabel: string;
  readonly busy: boolean;
  readonly disabled: boolean;
  readonly onPress: () => void;
}) {
  return (
    <Pressable
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      accessibilityState={{ disabled, busy }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        shared.compactAction,
        disabled && shared.disabled,
        pressed && shared.actionPressed,
      ]}
    >
      {busy ? (
        <ActivityIndicator color={colors.text} size="small" />
      ) : (
        <Text style={shared.compactActionLabel}>{label}</Text>
      )}
    </Pressable>
  );
}

function capitalize(state: string): string {
  return state.length > 0 ? state.charAt(0).toUpperCase() + state.slice(1) : state;
}

const styles = StyleSheet.create({
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
  detail: { color: colors.muted, fontSize: 13, lineHeight: 19 },
  dot: { borderRadius: 5, height: 10, width: 10 },
  identity: { alignItems: "center", flexDirection: "row", gap: 8, flex: 1 },
  image: { color: colors.text, fontSize: 14, lineHeight: 20 },
  name: { color: colors.text, fontSize: 17, fontWeight: "800", letterSpacing: -0.2 },
});
