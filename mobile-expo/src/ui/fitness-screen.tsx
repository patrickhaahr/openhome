import {
  ActivityIndicator,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { useEffect, useRef, useState } from "react";

import type { FitnessActions, FitnessState } from "../application/use-fitness";
import {
  filterExercises,
  parseExerciseInput,
  type Exercise,
  type ExerciseCategory,
} from "../domain/fitness";
import { ActionButton, PageHeading, styles as shared } from "./shared";
import { colors } from "./theme";

type LibraryCategory = ExerciseCategory | "all";

type FormState = {
  readonly name: string;
  readonly category: ExerciseCategory;
  readonly muscleGroup: string;
  readonly equipment: string;
};

const emptyForm: FormState = { name: "", category: "calisthenics", muscleGroup: "", equipment: "" };

const categoryFilters: ReadonlyArray<{ readonly key: LibraryCategory; readonly label: string }> = [
  { key: "all", label: "All" },
  { key: "calisthenics", label: "Calisthenics" },
  { key: "gym", label: "Gym" },
];

const categoryLabels: ReadonlyArray<{ readonly key: ExerciseCategory; readonly label: string }> = [
  { key: "calisthenics", label: "Calisthenics" },
  { key: "gym", label: "Gym" },
];

/** Render the Fitness Tab: the exercise library with search, filters, and a create form. */
export function FitnessPage({ state, actions }: { readonly state: FitnessState; readonly actions: FitnessActions }) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<LibraryCategory>("all");
  const [muscleGroup, setMuscleGroup] = useState("");
  const [formOpen, setFormOpen] = useState(false);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [formError, setFormError] = useState<string | null>(null);
  const wasBusy = useRef(false);

  // Clear the create form once its submission has succeeded; a failed create
  // keeps the fields so only the failing part has to change.
  useEffect(() => {
    if (state.tag !== "ready") {
      return;
    }
    if (wasBusy.current && !state.busy && state.error === null) {
      setForm(emptyForm);
      setFormError(null);
    }
    wasBusy.current = state.busy;
  }, [state]);

  const exercises = state.tag === "ready" ? state.exercises : [];
  const visible = filterExercises(exercises, query, category, muscleGroup);

  function submit(): void {
    const parsed = parseExerciseInput(form.name, form.category, form.muscleGroup, form.equipment);
    if (!parsed.ok) {
      setFormError(parsed.error);
      return;
    }
    setFormError(null);
    actions.create(parsed.value);
  }

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
          title="Exercise library"
          description="Every exercise you can log, searchable and filterable."
        />

        <View style={[shared.section, styles.card]}>
          <Pressable
            accessibilityLabel="Add an exercise"
            accessibilityRole="button"
            accessibilityState={{ expanded: formOpen }}
            onPress={() => setFormOpen((value) => !value)}
            style={({ pressed }) => [styles.formHeader, pressed && shared.iconPressed]}
          >
            <Text style={shared.sectionTitle}>ADD EXERCISE</Text>
            <Text style={styles.formToggle}>{formOpen ? "Hide" : "Show"}</Text>
          </Pressable>
          {formOpen ? (
            <>
              <TextInput
                accessibilityLabel="Exercise name"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(name) => setForm((current) => ({ ...current, name }))}
                placeholder="Name, e.g. Weighted Pull-up"
                placeholderTextColor={colors.muted}
                style={styles.input}
                value={form.name}
              />
              <View style={shared.row}>
                {categoryLabels.map(({ key, label }) => (
                  <Pressable
                    key={key}
                    accessibilityLabel={`${label} category`}
                    accessibilityRole="button"
                    accessibilityState={{ selected: form.category === key }}
                    onPress={() => setForm((current) => ({ ...current, category: key }))}
                    style={({ pressed }) => [
                      styles.chip,
                      form.category === key && styles.chipSelected,
                      pressed && shared.actionPressed,
                    ]}
                  >
                    <Text
                      style={[
                        styles.chipLabel,
                        form.category === key && styles.chipLabelSelected,
                      ]}
                    >
                      {label}
                    </Text>
                  </Pressable>
                ))}
              </View>
              <TextInput
                accessibilityLabel="Muscle group"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(muscleGroup) => setForm((current) => ({ ...current, muscleGroup }))}
                placeholder="Muscle group (optional)"
                placeholderTextColor={colors.muted}
                style={styles.input}
                value={form.muscleGroup}
              />
              <TextInput
                accessibilityLabel="Equipment"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(equipment) => setForm((current) => ({ ...current, equipment }))}
                placeholder="Equipment (optional)"
                placeholderTextColor={colors.muted}
                style={styles.input}
                value={form.equipment}
              />
              <View style={shared.row}>
                <ActionButton
                  label="Add exercise"
                  sending={state.tag === "ready" && state.busy}
                  disabled={state.tag !== "ready" || state.busy}
                  onPress={submit}
                />
              </View>
              {formError !== null ? (
                <Text accessibilityRole="alert" style={shared.error}>
                  {formError}
                </Text>
              ) : null}
              {state.tag === "ready" && state.error !== null ? (
                <Text accessibilityRole="alert" style={shared.error}>
                  {state.error}
                </Text>
              ) : null}
            </>
          ) : null}
        </View>

        <TextInput
          accessibilityLabel="Search exercises by name"
          autoCapitalize="none"
          autoCorrect={false}
          onChangeText={setQuery}
          placeholder="Search by name"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={query}
        />
        <View style={shared.row}>
          {categoryFilters.map(({ key, label }) => (
            <Pressable
              key={key}
              accessibilityLabel={`${label} category filter`}
              accessibilityRole="button"
              accessibilityState={{ selected: category === key }}
              onPress={() => setCategory(key)}
              style={({ pressed }) => [
                styles.chip,
                category === key && styles.chipSelected,
                pressed && shared.actionPressed,
              ]}
            >
              <Text style={[styles.chipLabel, category === key && styles.chipLabelSelected]}>
                {label}
              </Text>
            </Pressable>
          ))}
        </View>
        <TextInput
          accessibilityLabel="Filter exercises by muscle group"
          autoCapitalize="none"
          autoCorrect={false}
          onChangeText={setMuscleGroup}
          placeholder="Filter by muscle group"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={muscleGroup}
        />

        {state.tag === "loading" ? (
          <View style={shared.statusPanel}>
            <ActivityIndicator color={colors.signal} />
            <Text style={shared.statusText}>Loading exercises</Text>
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
              {exercises.length === 0 ? "No exercises yet" : "No exercises match"}
            </Text>
          </View>
        ) : null}
        {state.tag === "ready"
          ? visible.map((exercise) => <ExerciseRow key={exercise.id} exercise={exercise} />)
          : null}
        {state.tag === "ready" && state.error !== null && !formOpen ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.error}
          </Text>
        ) : null}
      </View>
    </ScrollView>
  );
}

function ExerciseRow({ exercise }: { readonly exercise: Exercise }) {
  return (
    <View style={shared.section}>
      <View style={shared.sectionHeader}>
        <Text style={styles.name}>{exercise.name}</Text>
        <Text style={shared.sectionDetail}>{exercise.category}</Text>
      </View>
      {exercise.muscleGroup !== null ? (
        <Text style={styles.detail}>Muscle group: {exercise.muscleGroup}</Text>
      ) : null}
      {exercise.equipment !== null ? (
        <Text style={styles.detail}>Equipment: {exercise.equipment}</Text>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
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
  detail: { color: colors.muted, fontSize: 13, lineHeight: 19 },
  formHeader: { minHeight: 24, flexDirection: "row", justifyContent: "space-between" },
  formToggle: { color: colors.signal, fontSize: 12, fontWeight: "800" },
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
});
