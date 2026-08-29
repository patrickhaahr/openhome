import {
  ActivityIndicator,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
} from "react-native";
import { useEffect, useState } from "react";

import type { AdguardActions, AdguardState } from "../application/use-adguard";
import type { DockerActions, DockerState } from "../application/use-docker";
import type { FeedsActions, FeedsState } from "../application/use-feeds";
import type { TimelineActions, TimelineState } from "../application/use-timeline";
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
  readonly timeline: TimelineState;
  readonly timelineActions: TimelineActions;
  readonly feeds: FeedsState;
  readonly feedsActions: FeedsActions;
  readonly onOpenTimelineLink: (url: string) => void;
  readonly onOpenDocker: () => void;
};

/** Render the Server Tab: mixed server-operations surfaces. */
export function ServerPage({
  adguard,
  actions,
  docker,
  dockerActions,
  dockerCounts,
  timeline,
  timelineActions,
  feeds,
  feedsActions,
  onOpenTimelineLink,
  onOpenDocker,
}: Props) {
  function handleScroll(event: NativeSyntheticEvent<NativeScrollEvent>): void {
    const { layoutMeasurement, contentOffset, contentSize } = event.nativeEvent;
    const nearBottom = layoutMeasurement.height + contentOffset.y >= contentSize.height - 400;
    if (nearBottom) {
      timelineActions.loadMore();
    }
  }

  return (
    <ScrollView
      contentContainerStyle={shared.pageContent}
      onScroll={handleScroll}
      refreshControl={
        <RefreshControl
          refreshing={
            (adguard.tag === "ready" && adguard.refreshing) ||
            (docker.tag === "ready" && docker.refreshing) ||
            (timeline.tag === "ready" && timeline.refreshing)
          }
          onRefresh={() => {
            actions.refresh();
            dockerActions.refresh();
            timelineActions.refresh();
            feedsActions.refresh();
          }}
          colors={[colors.signal]}
          tintColor={colors.signal}
        />
      }
      scrollEventThrottle={16}
    >
      <View style={shared.stack}>
        <PageHeading
          eyebrow="SERVER OPERATIONS"
          title="Server"
          description="Ad blocking, server health, and your feed timeline."
        />
        <AdGuardCard state={adguard} actions={actions} />
        <DockerSummaryCard
          state={docker}
          counts={dockerCounts}
          onPress={onOpenDocker}
        />
        <FeedManagerCard state={feeds} actions={feedsActions} />
        <TimelineCard
          state={timeline}
          actions={timelineActions}
          onOpenLink={onOpenTimelineLink}
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

/** Inline collapsible Feed manager: list, add by URL, delete with undo. */
function FeedManagerCard({
  state,
  actions,
}: {
  readonly state: FeedsState;
  readonly actions: FeedsActions;
}) {
  const [open, setOpen] = useState(false);

  return (
    <View style={[shared.section, styles.card]}>
      <Pressable
        accessibilityLabel="Manage feeds"
        accessibilityRole="button"
        accessibilityState={{ expanded: open }}
        onPress={() => setOpen((value) => !value)}
        style={({ pressed }) => [styles.feedsHeader, pressed && shared.iconPressed]}
      >
        <Text style={shared.sectionTitle}>MANAGE FEEDS</Text>
        <Text style={styles.feedsToggle}>{open ? "Hide" : "Show"}</Text>
      </Pressable>
      {open ? <FeedManagerBody state={state} actions={actions} /> : null}
    </View>
  );
}

function FeedManagerBody({
  state,
  actions,
}: {
  readonly state: FeedsState;
  readonly actions: FeedsActions;
}) {
  if (state.tag === "loading") {
    return (
      <View style={shared.statusPanel}>
        <ActivityIndicator color={colors.signal} />
        <Text style={shared.statusText}>Loading feeds</Text>
      </View>
    );
  }
  if (state.tag === "error") {
    return (
      <>
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
      </>
    );
  }

  return (
    <>
      {state.feeds.length === 0 ? (
        <Text style={styles.feedEmpty}>No feeds yet. Paste a URL below to add one.</Text>
      ) : (
        <View style={styles.feedList}>
          {state.feeds.map((feed) => (
            <View key={feed.id} style={styles.feedRow}>
              <View style={styles.feedCopy}>
                <Text numberOfLines={1} style={styles.feedTitle}>
                  {feed.title ?? feed.url}
                </Text>
                <Text numberOfLines={1} style={styles.feedUrl}>
                  {feed.url}
                </Text>
              </View>
              <Pressable
                accessibilityLabel={`Delete feed ${feed.title ?? feed.url}`}
                accessibilityRole="button"
                accessibilityState={{ disabled: state.busy }}
                disabled={state.busy}
                onPress={() => actions.remove(feed)}
                style={({ pressed }) => [
                  styles.feedDelete,
                  state.busy && shared.disabled,
                  pressed && shared.actionPressed,
                ]}
              >
                <Text style={styles.feedDeleteLabel}>Delete</Text>
              </Pressable>
            </View>
          ))}
        </View>
      )}
      {state.undoable.length > 0 ? (
        <Pressable
          accessibilityLabel={`Undo delete of ${state.undoable.length} ${
            state.undoable.length === 1 ? "feed" : "feeds"
          }`}
          accessibilityRole="button"
          accessibilityState={{ disabled: state.busy }}
          disabled={state.busy}
          onPress={actions.undo}
          style={({ pressed }) => [styles.feedUndo, pressed && shared.actionPressed]}
        >
          <Text style={shared.retry}>
            Undo delete{state.undoable.length > 1 ? ` (${state.undoable.length})` : ""}
          </Text>
        </Pressable>
      ) : null}
      <TextInput
        accessibilityLabel="Feed URL to add"
        autoCapitalize="none"
        autoCorrect={false}
        keyboardType="url"
        onChangeText={actions.setInput}
        placeholder="Paste a feed URL"
        placeholderTextColor={colors.muted}
        style={styles.feedInput}
        value={state.input}
      />
      <View style={shared.row}>
        <ActionButton label="Add feed" sending={state.busy} disabled={state.busy} onPress={actions.create} />
      </View>
      {state.error !== null ? (
        <Text accessibilityRole="alert" style={shared.error}>
          {state.error}
        </Text>
      ) : null}
    </>
  );
}

/** Presentation for the Compact Timeline: a skim list, not an article reader. */
function TimelineCard({
  state,
  actions,
  onOpenLink,
}: {
  readonly state: TimelineState;
  readonly actions: TimelineActions;
  readonly onOpenLink: (url: string) => void;
}) {
  if (state.tag === "loading") {
    return (
      <View style={[shared.section, styles.card]}>
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Loading timeline</Text>
        </View>
      </View>
    );
  }
  if (state.tag === "error") {
    return (
      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>COMPACT TIMELINE</Text>
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
    );
  }

  return (
    <View style={[shared.section, styles.card]}>
      <Text style={shared.sectionTitle}>COMPACT TIMELINE</Text>
      {state.items.length === 0 ? (
        <Text style={styles.timelineEmpty}>No feed items yet.</Text>
      ) : (
        <View style={styles.timeline}>
          {state.items.map((item) => (
            <Pressable
              key={item.id}
              accessibilityLabel={`Open ${item.title}`}
              accessibilityRole="link"
              onPress={() => onOpenLink(item.link)}
              style={({ pressed }) => [styles.timelineItem, pressed && shared.iconPressed]}
            >
              <Text numberOfLines={2} style={styles.timelineTitle}>
                {item.title}
              </Text>
              {item.description !== null ? (
                <Text numberOfLines={1} style={styles.timelineDescription}>
                  {item.description}
                </Text>
              ) : null}
            </Pressable>
          ))}
        </View>
      )}
      {state.loadingMore ? (
        <View style={styles.moreRow}>
          <ActivityIndicator color={colors.signal} size="small" />
          <Text style={shared.sectionDetail}>Loading older items</Text>
        </View>
      ) : null}
      {state.error !== null ? (
        <>
          <Text accessibilityRole="alert" style={shared.error}>
            {state.error}
          </Text>
          <Pressable
            accessibilityRole="button"
            onPress={actions.loadMore}
            style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
          >
            <Text style={shared.retry}>Try again</Text>
          </Pressable>
        </>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  card: { gap: 16 },
  feedCopy: { flex: 1, gap: 2 },
  feedDelete: {
    alignSelf: "center",
    borderColor: colors.border,
    borderRadius: 10,
    borderWidth: 1,
    justifyContent: "center",
    minHeight: 40,
    paddingHorizontal: 14,
  },
  feedDeleteLabel: { color: colors.danger, fontSize: 13, fontWeight: "700" },
  feedEmpty: { color: colors.muted, fontSize: 14, lineHeight: 20 },
  feedInput: {
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
  feedList: { gap: 12 },
  feedRow: { alignItems: "center", flexDirection: "row", gap: 10 },
  feedTitle: { color: colors.text, fontSize: 15, fontWeight: "700", lineHeight: 20 },
  feedUndo: { minHeight: 44, justifyContent: "center" },
  feedUrl: { color: colors.muted, fontSize: 13, lineHeight: 18 },
  feedsHeader: { minHeight: 24, flexDirection: "row", justifyContent: "space-between" },
  feedsToggle: { color: colors.signal, fontSize: 12, fontWeight: "800" },
  moreRow: { alignItems: "center", flexDirection: "row", gap: 8, justifyContent: "center" },
  phaseCopy: { flex: 1, gap: 2 },
  phaseDetail: { color: colors.muted, fontSize: 14, lineHeight: 20 },
  phaseDot: { borderRadius: 8, height: 15, marginTop: 3, width: 15 },
  phaseLabel: { color: colors.text, fontSize: 21, fontWeight: "800", letterSpacing: -0.4 },
  phaseRow: { alignItems: "flex-start", flexDirection: "row", gap: 12 },
  runningDot: { borderRadius: 3, height: 6, width: 6 },
  timeline: { gap: 14 },
  timelineDescription: { color: colors.muted, fontSize: 13, lineHeight: 18 },
  timelineEmpty: { color: colors.muted, fontSize: 14, lineHeight: 20 },
  timelineItem: { gap: 2 },
  timelineTitle: { color: colors.text, fontSize: 15, fontWeight: "700", lineHeight: 20 },
  versionRow: { alignItems: "center", flexDirection: "row", gap: 5 },
});
