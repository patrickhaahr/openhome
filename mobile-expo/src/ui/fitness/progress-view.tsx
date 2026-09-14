import { ActivityIndicator, Pressable, Text, View } from "react-native";

import { LineChart } from "react-native-gifted-charts";

import type { ProgressActions, ProgressState } from "../../application/use-progress";
import type { Exercise, ProgressPoint } from "../../domain/fitness";
import { styles as shared } from "../shared";
import { colors } from "../theme";
import { formatDate, groupByMuscleGroup, styles } from "./shared";

/** Render the per-exercise progress view: an exercise picker and its trend series. */
export function ProgressView({
  state,
  actions,
  exercises,
}: {
  readonly state: ProgressState;
  readonly actions: ProgressActions;
  readonly exercises: readonly Exercise[];
}) {
  const selectedId =
    state.tag === "loaded"
      ? state.progress.exercise.id
      : state.tag === "loading" || state.tag === "error"
        ? state.exerciseId
        : null;
  return (
    <>
      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>PICK AN EXERCISE</Text>
        {exercises.length === 0 ? (
          <Text style={styles.detail}>
            The exercise library is empty. Add exercises on the Library view.
          </Text>
        ) : (
          groupByMuscleGroup(exercises).map(([group, groupExercises]) => (
            <View key={group} style={shared.stack}>
              <Text style={styles.groupLabel}>{group.toUpperCase()}</Text>
              <View style={styles.chipWrap}>
                {groupExercises.map((exercise) => (
                  <Pressable
                    key={exercise.id}
                    accessibilityLabel={`Show progress for ${exercise.name}`}
                    accessibilityRole="button"
                    accessibilityState={{ selected: selectedId === exercise.id }}
                    onPress={() => actions.openExercise(exercise.id)}
                    style={({ pressed }) => [
                      styles.pickChip,
                      selectedId === exercise.id && styles.chipSelected,
                      pressed && shared.actionPressed,
                    ]}
                  >
                    <Text
                      style={[
                        styles.chipLabel,
                        selectedId === exercise.id && styles.chipLabelSelected,
                      ]}
                    >
                      {exercise.name}
                    </Text>
                  </Pressable>
                ))}
              </View>
            </View>
          ))
        )}
      </View>
      {state.tag === "loading" ? (
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Loading progress</Text>
        </View>
      ) : null}
      {state.tag === "idle" ? (
        <View style={shared.statusPanel}>
          <Text style={shared.statusText}>{state.message}</Text>
        </View>
      ) : null}
      {state.tag === "error" ? (
        <View style={shared.section}>
          <Text accessibilityRole="alert" style={shared.error}>
            {state.message}
          </Text>
          {selectedId !== null ? (
            <Pressable
              accessibilityRole="button"
              onPress={actions.refresh}
              style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
            >
              <Text style={shared.retry}>Try again</Text>
            </Pressable>
          ) : null}
        </View>
      ) : null}
      {state.tag === "loaded" ? (
        state.progress.data.length === 0 ? (
          <View style={shared.statusPanel}>
            <Text style={shared.statusText}>No logged sets for {state.progress.exercise.name} yet</Text>
          </View>
        ) : (
          <>
            <View style={shared.section}>
              <Text style={styles.name}>{state.progress.exercise.name}</Text>
            </View>
            <ProgressChart data={state.progress.data} />
            {state.progress.data.map((point) => (
              <ProgressPointRow key={point.date} point={point} />
            ))}
          </>
        )
      ) : null}
    </>
  );
}

/** Estimated 1RM per workout date as a line chart; text rows below keep the exact numbers. */
function ProgressChart({ data }: { readonly data: readonly ProgressPoint[] }) {
  const chartData = data.flatMap((point) =>
    point.estimated1RmKg === null
      ? []
      : [
          {
            value: point.estimated1RmKg,
            label: point.date.slice(5),
            dataPointText: point.estimated1RmKg.toFixed(1),
          },
        ],
  );
  return (
    <View style={shared.section}>
      <Text style={shared.sectionTitle}>ESTIMATED 1RM (KG)</Text>
      {chartData.length === 0 ? (
        <Text style={styles.detail}>No weighted sets yet — 1RM needs reps and weight.</Text>
      ) : (
        <LineChart
          data={chartData}
          curved
          areaChart
          color={colors.signal}
          startFillColor={colors.signal}
          endFillColor={colors.signalDark}
          startOpacity={0.6}
          endOpacity={0.1}
          dataPointsColor={colors.signal}
          textColor={colors.text}
          textFontSize={11}
          yAxisColor={colors.border}
          xAxisColor={colors.border}
          yAxisTextStyle={{ color: colors.muted, fontSize: 11 }}
          xAxisLabelTextStyle={{ color: colors.muted, fontSize: 10 }}
          noOfSections={4}
          spacing={48}
          rulesColor={colors.border}
          pointerConfig={{
            pointerStripColor: colors.border,
            pointerStripWidth: 2,
            pointerColor: colors.signal,
            radius: 4,
            activatePointersOnLongPress: true,
          }}
        />
      )}
    </View>
  );
}

/** One date's aggregated progress as a text row with the exact numbers under the chart. */
function ProgressPointRow({ point }: { readonly point: ProgressPoint }) {
  return (
    <View style={shared.section}>
      <Text style={styles.name}>{formatDate(point.date)}</Text>
      <Text style={styles.detail}>{describePoint(point)}</Text>
    </View>
  );
}

/** Summarize one progress point's metrics, dropping the ones without data. */
function describePoint(point: ProgressPoint): string {
  const parts: string[] = [];
  if (point.bestWeightKg !== null) {
    parts.push(`Best ${point.bestWeightKg} kg`);
  }
  if (point.bestReps !== null) {
    parts.push(`${point.bestReps} reps`);
  }
  if (point.totalVolumeKg !== null) {
    parts.push(`Volume ${point.totalVolumeKg} kg`);
  }
  if (point.estimated1RmKg !== null) {
    parts.push(`Est. 1RM ${point.estimated1RmKg} kg`);
  }
  if (point.bestRpe !== null) {
    parts.push(`RPE ${point.bestRpe}`);
  }
  return parts.join(" · ") || "No set data";
}
