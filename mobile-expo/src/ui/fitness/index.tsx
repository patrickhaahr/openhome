import { Pressable, RefreshControl, ScrollView, Text, View } from "react-native";
import { useState } from "react";

import type { BodyActions, BodyState } from "../../application/use-body";
import type { FitnessActions, FitnessState } from "../../application/use-fitness";
import type { ProgressActions, ProgressState } from "../../application/use-progress";
import type { WorkoutsActions, WorkoutsState } from "../../application/use-workouts";
import { PageHeading, styles as shared } from "../shared";
import { colors } from "../theme";
import { BodyView } from "./body-view";
import { LibraryView } from "./library-view";
import { ProgressView } from "./progress-view";
import { styles } from "./shared";
import { WorkoutsView } from "./workouts-view";

/** Which of the Fitness Tab's four views is showing. */
type FitnessView = "library" | "workouts" | "progress" | "body";

const viewLabels: ReadonlyArray<{ readonly key: FitnessView; readonly label: string }> = [
  { key: "workouts", label: "Workouts" },
  { key: "library", label: "Library" },
  { key: "progress", label: "Progress" },
  { key: "body", label: "Body" },
];

const viewHeadings = {
  library: {
    title: "Exercise library",
    description: "Every exercise you can log, searchable and filterable.",
  },
  workouts: { title: "Workouts", description: "Log sessions and review what you did." },
  progress: {
    title: "Progress",
    description: "Pick an exercise and see its history over time.",
  },
  body: {
    title: "Body",
    description: "Log your weight and configure your profile.",
  },
} satisfies Record<FitnessView, { title: string; description: string }>;

/** Render the Fitness Tab: the exercise library, workout log, progress, and body views. */
export function FitnessPage({
  state,
  actions,
  workouts,
  workoutsActions,
  progress,
  progressActions,
  body,
  bodyActions,
}: {
  readonly state: FitnessState;
  readonly actions: FitnessActions;
  readonly workouts: WorkoutsState;
  readonly workoutsActions: WorkoutsActions;
  readonly progress: ProgressState;
  readonly progressActions: ProgressActions;
  readonly body: BodyState;
  readonly bodyActions: BodyActions;
}) {
  const [view, setView] = useState<FitnessView>("workouts");

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
          eyebrow="FITNESS"
          title={viewHeadings[view].title}
          description={viewHeadings[view].description}
        />
        <View style={shared.row}>
          {viewLabels.map(({ key, label }) => (
            <Pressable
              key={key}
              accessibilityLabel={`${label} view`}
              accessibilityRole="tab"
              accessibilityState={{ selected: view === key }}
              onPress={() => setView(key)}
              style={({ pressed }) => [
                styles.chip,
                view === key && styles.chipSelected,
                pressed && shared.actionPressed,
              ]}
            >
              <Text style={[styles.chipLabel, view === key && styles.chipLabelSelected]}>
                {label}
              </Text>
            </Pressable>
          ))}
        </View>
        {view === "library" ? <LibraryView state={state} actions={actions} /> : null}
        {view === "workouts" ? (
          <WorkoutsView
            state={workouts}
            actions={workoutsActions}
            exercises={state.tag === "ready" ? state.exercises : []}
          />
        ) : null}
        {view === "progress" ? (
          <ProgressView
            state={progress}
            actions={progressActions}
            exercises={state.tag === "ready" ? state.exercises : []}
          />
        ) : null}
        {view === "body" ? <BodyView state={body} actions={bodyActions} /> : null}
      </View>
    </ScrollView>
  );
}
