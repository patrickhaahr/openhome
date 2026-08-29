import {
  ActivityIndicator,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useEffect, useState } from "react";

import type { AdguardActions, AdguardState } from "../application/use-adguard";
import type { DockerActions, DockerState } from "../application/use-docker";
import {
  dockerHealthSummary,
  type ClassificationCounts,
  type DockerSummaryClassification,
} from "../domain/docker";
import {
  formatPauseRemaining,
  pauseOptionsMinutes,
  protectionPhase,
  type ProtectionPhase,
} from "../domain/adguard";
import { ActionButton, PageHeading, SecondaryAction, styles as shared } from "./shared";
import { colors } from "./theme";

type Props = {
  readonly adguard: AdguardState;
  readonly actions: AdguardActions;
  readonly docker: DockerState;
  readonly dockerActions: DockerActions;
  readonly dockerCounts: ClassificationCounts;
  readonly onOpenDocker: () => void;
};

/** Render the Server Tab: mixed server-operations surfaces. */
export function ServerPage({ adguard, actions, docker, dockerActions, dockerCounts, onOpenDocker }: Props) {
  return (
    <ScrollView
      contentContainerStyle={shared.pageContent}
      refreshControl={
        <RefreshControl
          refreshing={
            (adguard.tag === "ready" && adguard.refreshing) ||
            (docker.tag === "ready" && docker.refreshing)
          }
          onRefresh={() => {
            actions.refresh();
            dockerActions.refresh();
          }}
          colors={[colors.signal]}
          tintColor={colors.signal}
        />
      }
    >
      <View style={shared.stack}>
        <PageHeading
          eyebrow="SERVER OPERATIONS"
          title="Server"
          description="Ad blocking and server health at a glance."
        />
        <AdGuardCard state={adguard} actions={actions} />
        <DockerSummaryCard
          state={docker}
          counts={dockerCounts}
          onPress={onOpenDocker}
        />
      </View>
    </ScrollView>
  );
}

function AdGuardCard({
  state,
  actions,
}: {
  readonly state: AdguardState;
  readonly actions: AdguardActions;
}) {
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [pauseOpen, setPauseOpen] = useState(false);

  useEffect(() => {
    if (state.tag !== "ready" || state.status.pauseEndsAtMs === null) {
      return;
    }
    const endsAt = state.status.pauseEndsAtMs;
    const timer = setInterval(() => {
      const now = Date.now();
      setNowMs(now);
      if (endsAt <= now) {
        clearInterval(timer);
        actions.refresh();
      }
    }, 1000);
    return () => clearInterval(timer);
  }, [state, actions]);

  if (state.tag === "loading") {
    return (
      <View style={[shared.section, styles.card]}>
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Checking AdGuard</Text>
        </View>
      </View>
    );
  }
  if (state.tag === "error") {
    return (
      <View style={[shared.section, styles.card]}>
        <Text accessibilityRole="alert" style={shared.error}>
          {state.message}
        </Text>
        <Pressable
          accessibilityRole="button"
          onPress={() => {
            setPauseOpen(false);
            actions.refresh();
          }}
          style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
        >
          <Text style={shared.retry}>Try again</Text>
        </Pressable>
      </View>
    );
  }

  const phase = protectionPhase(state.status, nowMs);
  const acting = state.acting;
  return (
    <View style={[shared.section, styles.card]}>
      <View style={shared.sectionHeader}>
        <Text style={shared.sectionTitle}>ADGUARD PROTECTION</Text>
        <View style={styles.versionRow}>
          <View
            style={[
              styles.runningDot,
              { backgroundColor: state.status.running ? colors.ready : colors.danger },
            ]}
          />
          <Text style={shared.sectionDetail}>
            {state.status.version} · {state.status.running ? "Running" : "Not running"}
          </Text>
        </View>
      </View>

      <View style={styles.phaseRow}>
        <View style={[styles.phaseDot, { backgroundColor: phasePresentation[phase].color }]} />
        <View style={styles.phaseCopy}>
          <Text style={styles.phaseLabel}>{phasePresentation[phase].label}</Text>
          {phase === "paused" ? (
            <Text style={styles.phaseDetail}>
              Filtering resumes in{" "}
              {formatPauseRemaining((state.status.pauseEndsAtMs ?? nowMs) - nowMs)}.
            </Text>
          ) : (
            <Text style={styles.phaseDetail}>
              {state.status.running
                ? "DNS filtering across your network is controlled here."
                : "AdGuard Home itself isn't responding."}
            </Text>
          )}
        </View>
      </View>

      {pauseOpen ? (
        <>
          <View style={shared.row}>
            {pauseOptionsMinutes.map((minutes) => (
              <Pressable
                key={minutes}
                accessibilityLabel={`Pause AdGuard for ${minutes} minutes`}
                accessibilityRole="button"
                accessibilityState={{ disabled: acting }}
                disabled={acting}
                onPress={() => {
                  setPauseOpen(false);
                  actions.pause(minutes);
                }}
                style={({ pressed }) => [
                  shared.compactAction,
                  acting && shared.disabled,
                  pressed && shared.actionPressed,
                ]}
              >
                <Text style={shared.compactActionLabel}>{minutes} min</Text>
              </Pressable>
            ))}
          </View>
          <Pressable
            accessibilityRole="button"
            onPress={() => {
              setPauseOpen(false);
            }}
            style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
          >
            <Text style={shared.retry}>Cancel</Text>
          </Pressable>
        </>
      ) : state.status.protectionEnabled ? (
        <View style={shared.row}>
          <SecondaryAction
            label="Disable"
            disabled={acting}
            onPress={() => {
              setPauseOpen(false);
              actions.disable();
            }}
          />
          <SecondaryAction
            label="Pause…"
            disabled={acting}
            onPress={() => {
              setPauseOpen(true);
            }}
          />
        </View>
      ) : (
        <View style={shared.row}>
          <ActionButton
            label="Enable"
            sending={acting}
            disabled={acting}
            onPress={() => {
              setPauseOpen(false);
              actions.enable();
            }}
          />
        </View>
      )}

      {state.error !== null ? (
        <Text accessibilityRole="alert" style={shared.error}>
          {state.error}
        </Text>
      ) : null}
    </View>
  );
}

const phasePresentation = {
  protected: { color: colors.ready, label: "Protected" },
  paused: { color: colors.signal, label: "Paused" },
  unprotected: { color: colors.danger, label: "Unprotected" },
} satisfies Record<ProtectionPhase, { label: string; color: string }>;

/** Presentation for the Docker Health Summary headline, offline included. */
const summaryPresentation = {
  healthy: { color: colors.ready, label: "Docker healthy" },
  unhealthy: { color: colors.danger, label: "Docker unhealthy" },
  idle: { color: colors.muted, label: "Docker idle" },
  offline: { color: colors.danger, label: "Docker offline" },
} satisfies Record<DockerSummaryClassification | "offline", { label: string; color: string }>;

function DockerSummaryCard({
  state,
  counts,
  onPress,
}: {
  readonly state: DockerState;
  readonly counts: ClassificationCounts;
  readonly onPress: () => void;
}) {
  if (state.tag === "loading") {
    return (
      <View style={[shared.section, styles.card]}>
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Checking containers</Text>
        </View>
      </View>
    );
  }

  const classification =
    state.tag === "error" ? "offline" : dockerHealthSummary(state.containers);
  const detail =
    state.tag === "error"
      ? state.message
      : classification === "idle"
        ? "No containers running."
        : `${counts.healthy + counts.unhealthy} of ${counts.all} running.`;
  const presentation = summaryPresentation[classification];

  return (
    <Pressable
      accessibilityLabel={`Docker Health Summary: ${presentation.label}`}
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => [shared.section, styles.card, pressed && shared.actionPressed]}
    >
      <View style={styles.phaseRow}>
        <View style={[styles.phaseDot, { backgroundColor: presentation.color }]} />
        <View style={styles.phaseCopy}>
          <Text style={styles.phaseLabel}>{presentation.label}</Text>
          <Text style={styles.phaseDetail}>{detail}</Text>
        </View>
      </View>
      {state.tag === "ready" && state.error !== null ? (
        <Text accessibilityRole="alert" style={shared.error}>
          {state.error}
        </Text>
      ) : null}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  card: { gap: 16 },
  phaseCopy: { flex: 1, gap: 2 },
  phaseDetail: { color: colors.muted, fontSize: 14, lineHeight: 20 },
  phaseDot: { borderRadius: 8, height: 15, marginTop: 3, width: 15 },
  phaseLabel: { color: colors.text, fontSize: 21, fontWeight: "800", letterSpacing: -0.4 },
  phaseRow: { alignItems: "flex-start", flexDirection: "row", gap: 12 },
  runningDot: { borderRadius: 3, height: 6, width: 6 },
  versionRow: { alignItems: "center", flexDirection: "row", gap: 5 },
});
