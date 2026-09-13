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
import type { WorkoutsActions, WorkoutsState } from "../application/use-workouts";
import {
  filterExercises,
  parseExerciseInput,
  parseWorkoutInput,
  type Exercise,
  type ExerciseCategory,
  type SetInput,
  type WorkoutDetail,
  type WorkoutExerciseEntry,
  type WorkoutExerciseInput,
} from "../domain/fitness";
import { ActionButton, PageHeading, SecondaryAction, styles as shared } from "./shared";
import { colors } from "./theme";

type LibraryCategory = ExerciseCategory | "all";
/** Which of the Fitness Tab's two views is showing. */
type FitnessView = "library" | "workouts";

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

const viewLabels: ReadonlyArray<{ readonly key: FitnessView; readonly label: string }> = [
  { key: "library", label: "Library" },
  { key: "workouts", label: "Workouts" },
];

/** Render the Fitness Tab: the exercise library and the workout log/history. */
export function FitnessPage({
  state,
  actions,
  workouts,
  workoutsActions,
}: {
  readonly state: FitnessState;
  readonly actions: FitnessActions;
  readonly workouts: WorkoutsState;
  readonly workoutsActions: WorkoutsActions;
}) {
  const [view, setView] = useState<FitnessView>("library");

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
          title={view === "library" ? "Exercise library" : "Workouts"}
          description={
            view === "library"
              ? "Every exercise you can log, searchable and filterable."
              : "Log sessions and review what you did."
          }
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
      </View>
    </ScrollView>
  );
}

function LibraryView({
  state,
  actions,
}: {
  readonly state: FitnessState;
  readonly actions: FitnessActions;
}) {
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
    <>
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
                      style={[styles.chipLabel, form.category === key && styles.chipLabelSelected]}
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
    </>
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

/** Today's date as the log form's YYYY-MM-DD default. */
function today(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

function formatDate(date: string): string {
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

function describeSet(set: SetInput): string {
  const parts: string[] = [];
  if (set.reps !== null) {
    parts.push(`${set.reps} reps`);
  }
  if (set.durationSeconds !== null) {
    parts.push(`${set.durationSeconds}s hold`);
  }
  if (set.weightKg !== null) {
    parts.push(`+${set.weightKg} kg`);
  }
  if (set.rpe !== null) {
    parts.push(`RPE ${set.rpe}`);
  }
  return parts.join(" · ");
}

type WorkoutFormState = {
  readonly date: string;
  readonly name: string;
  readonly notes: string;
  readonly bodyWeightKg: string;
  readonly entries: ReadonlyArray<{
    readonly exerciseId: number;
    readonly exerciseName: string;
    readonly notes: string;
    readonly sets: ReadonlyArray<{
      reps: string;
      weightKg: string;
      durationSeconds: string;
      rpe: string;
      notes: string;
    }>;
  }>;
};

const emptySetRow = { reps: "", weightKg: "", durationSeconds: "", rpe: "", notes: "" };

function formFromDetail(workout: WorkoutDetail): WorkoutFormState {
  return {
    date: workout.date,
    name: workout.name ?? "",
    notes: workout.notes ?? "",
    bodyWeightKg: workout.bodyWeightKg === null ? "" : String(workout.bodyWeightKg),
    entries: workout.exercises.map((entry) => ({
      exerciseId: entry.exercise.id,
      exerciseName: entry.exercise.name,
      notes: entry.notes ?? "",
      sets: entry.sets.map((set) => ({
        reps: set.reps === null ? "" : String(set.reps),
        weightKg: set.weightKg === null ? "" : String(set.weightKg),
        durationSeconds: set.durationSeconds === null ? "" : String(set.durationSeconds),
        rpe: set.rpe === null ? "" : String(set.rpe),
        notes: set.notes ?? "",
      })),
    })),
  };
}

/** Collect the form into domain inputs; sets carry their 1-based numbers. */
function collectEntries(form: WorkoutFormState): WorkoutExerciseInput[] {
  return form.entries.map((entry) => ({
    exerciseId: entry.exerciseId,
    notes: entry.notes,
    sets: entry.sets.map((set, index) => ({
      setNumber: index + 1,
      reps: set.reps.trim().length > 0 ? Number(set.reps) : null,
      weightKg: set.weightKg.trim().length > 0 ? Number(set.weightKg) : null,
      durationSeconds: set.durationSeconds.trim().length > 0 ? Number(set.durationSeconds) : null,
      rpe: set.rpe.trim().length > 0 ? Number(set.rpe) : null,
      notes: set.notes,
    })),
  }));
}

function WorkoutsView({
  state,
  actions,
  exercises,
}: {
  readonly state: WorkoutsState;
  readonly actions: WorkoutsActions;
  readonly exercises: readonly Exercise[];
}) {
  const [form, setForm] = useState<WorkoutFormState | null>(null);
  // Which workout id the open form edits; null means the form creates.
  const [editingId, setEditingId] = useState<number | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const wasSaving = useRef(false);

  useEffect(() => {
    // Close the form on the saving -> not-saving transition with no error (a
    // succeeded save); a failed save keeps the fields so only the failing part
    // has to change. Static state cannot distinguish "just opened" from
    // "just succeeded", so the previous saving flag is what gates the close.
    if (wasSaving.current && !state.saving && state.error === null) {
      setForm(null);
      setEditingId(null);
      setFormError(null);
    }
    wasSaving.current = state.saving;
  }, [state]);

  function openCreate(): void {
    setEditingId(null);
    setForm({ date: today(), name: "", notes: "", bodyWeightKg: "", entries: [] });
    setFormError(null);
  }

  function openEdit(workout: WorkoutDetail): void {
    setEditingId(workout.id);
    setForm(formFromDetail(workout));
    setFormError(null);
  }

  function closeForm(): void {
    setForm(null);
    setEditingId(null);
    setFormError(null);
  }

  function submit(): void {
    if (form === null) {
      return;
    }
    const parsed = parseWorkoutInput(
      form.date,
      form.name,
      form.notes,
      form.bodyWeightKg,
      collectEntries(form),
    );
    if (!parsed.ok) {
      setFormError(parsed.error);
      return;
    }
    setFormError(null);
    if (editingId === null) {
      actions.saveWorkout({ tag: "create", input: parsed.value });
    } else {
      actions.saveWorkout({ tag: "update", id: editingId, input: parsed.value });
    }
  }

  if (form !== null) {
    return (
      <WorkoutForm
        form={form}
        setForm={(update) => setForm((current) => (current === null ? current : update(current)))}
        exercises={exercises}
        error={formError ?? state.error}
        saving={state.saving}
        editing={editingId !== null}
        onSubmit={submit}
        onClose={closeForm}
      />
    );
  }

  if (state.detail.tag === "closed") {
    return (
      <HistoryList
        state={state}
        onOpen={actions.openWorkout}
        onCreate={openCreate}
        onRefresh={actions.refresh}
      />
    );
  }

  if (state.detail.tag === "loading") {
    return (
      <View style={shared.statusPanel}>
        <ActivityIndicator color={colors.signal} />
        <Text style={shared.statusText}>Loading workout</Text>
      </View>
    );
  }

  if (state.detail.tag !== "loaded") {
    return (
      <View style={shared.section}>
        <Text accessibilityRole="alert" style={shared.error}>
          {state.detail.tag === "error" ? state.detail.message : "Workout unavailable."}
        </Text>
        <Pressable
          accessibilityRole="button"
          onPress={actions.closeWorkout}
          style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
        >
          <Text style={shared.retry}>Back to history</Text>
        </Pressable>
      </View>
    );
  }

  const workout = state.detail.workout;
  return (
    <WorkoutDetailView
      workout={workout}
      busy={state.saving}
      onEdit={() => openEdit(workout)}
      onDelete={() => actions.deleteWorkout(workout.id)}
      onBack={actions.closeWorkout}
    />
  );
}

function HistoryList({
  state,
  onOpen,
  onCreate,
  onRefresh,
}: {
  readonly state: WorkoutsState;
  readonly onOpen: (id: number) => void;
  readonly onCreate: () => void;
  readonly onRefresh: () => void;
}) {
  return (
    <>
      <View style={shared.row}>
        <ActionButton label="Log workout" sending={false} disabled={false} onPress={onCreate} />
      </View>
      {state.refreshing ? (
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Refreshing</Text>
        </View>
      ) : null}
      {state.workouts.length === 0 && !state.refreshing ? (
        <View style={shared.statusPanel}>
          <Text style={shared.statusText}>
            {state.error === null ? "No workouts yet" : state.error}
          </Text>
        </View>
      ) : null}
      {state.workouts.map((workout) => (
        <Pressable
          key={workout.id}
          accessibilityLabel={`Open workout on ${formatDate(workout.date)}`}
          accessibilityRole="button"
          onPress={() => onOpen(workout.id)}
          style={({ pressed }) => [shared.section, pressed && shared.iconPressed]}
        >
          <View style={shared.sectionHeader}>
            <Text style={workoutStyles.workoutDate}>{formatDate(workout.date)}</Text>
            <Text style={workoutStyles.workoutOpen}>Open</Text>
          </View>
          {workout.name !== null ? (
            <Text style={workoutStyles.workoutName}>{workout.name}</Text>
          ) : null}
        </Pressable>
      ))}
      <Pressable
        accessibilityRole="button"
        onPress={onRefresh}
        style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
      >
        <Text style={shared.retry}>Refresh history</Text>
      </Pressable>
      {state.error !== null && state.workouts.length > 0 ? (
        <Text accessibilityRole="alert" style={shared.error}>
          {state.error}
        </Text>
      ) : null}
    </>
  );
}

function WorkoutDetailView({
  workout,
  busy,
  onEdit,
  onDelete,
  onBack,
}: {
  readonly workout: WorkoutDetail;
  readonly busy: boolean;
  readonly onEdit: () => void;
  readonly onDelete: () => void;
  readonly onBack: () => void;
}) {
  return (
    <>
      <Pressable
        accessibilityRole="button"
        onPress={onBack}
        style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
      >
        <Text style={shared.retry}>‹ Back to history</Text>
      </Pressable>
      <View style={shared.section}>
        <Text style={workoutStyles.workoutDate}>{formatDate(workout.date)}</Text>
        {workout.name !== null ? (
          <Text style={workoutStyles.workoutName}>{workout.name}</Text>
        ) : null}
        {workout.bodyWeightKg !== null ? (
          <Text style={styles.detail}>Body weight: {workout.bodyWeightKg} kg</Text>
        ) : null}
        {workout.notes !== null ? <Text style={styles.detail}>{workout.notes}</Text> : null}
        <View style={shared.row}>
          <ActionButton label="Edit" sending={false} disabled={busy} onPress={onEdit} />
          <SecondaryAction label="Delete" disabled={busy} onPress={onDelete} />
        </View>
      </View>
      {workout.exercises.map((entry) => (
        <WorkoutExerciseBlock key={entry.id} entry={entry} />
      ))}
    </>
  );
}

function WorkoutExerciseBlock({ entry }: { readonly entry: WorkoutExerciseEntry }) {
  return (
    <View style={shared.section}>
      <View style={shared.sectionHeader}>
        <Text style={styles.name}>{entry.exercise.name}</Text>
        <Text style={shared.sectionDetail}>{entry.exercise.category}</Text>
      </View>
      {entry.notes !== null ? <Text style={styles.detail}>{entry.notes}</Text> : null}
      {entry.sets.map((set) => (
        <View key={set.id} style={workoutStyles.setRow}>
          <Text style={workoutStyles.setNumber}>{set.setNumber}</Text>
          <Text style={workoutStyles.setDetail}>{describeSet(set)}</Text>
          {set.notes !== null ? <Text style={workoutStyles.setNotes}>{set.notes}</Text> : null}
        </View>
      ))}
    </View>
  );
}

function WorkoutForm({
  form,
  setForm,
  exercises,
  error,
  saving,
  editing,
  onSubmit,
  onClose,
}: {
  readonly form: WorkoutFormState;
  readonly setForm: (update: (current: WorkoutFormState) => WorkoutFormState) => void;
  readonly exercises: readonly Exercise[];
  readonly error: string | null;
  readonly saving: boolean;
  readonly editing: boolean;
  readonly onSubmit: () => void;
  readonly onClose: () => void;
}) {
  return (
    <>
      <Pressable
        accessibilityRole="button"
        onPress={onClose}
        style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
      >
        <Text style={shared.retry}>‹ {editing ? "Back to workout" : "Back to history"}</Text>
      </Pressable>
      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>{editing ? "EDIT WORKOUT" : "LOG WORKOUT"}</Text>
        <TextInput
          accessibilityLabel="Workout date"
          autoCapitalize="none"
          autoCorrect={false}
          onChangeText={(date) => setForm((current) => ({ ...current, date }))}
          placeholder="YYYY-MM-DD"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={form.date}
        />
        <TextInput
          accessibilityLabel="Workout name"
          autoCapitalize="none"
          autoCorrect={false}
          onChangeText={(name) => setForm((current) => ({ ...current, name }))}
          placeholder="Name (optional), e.g. Push A"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={form.name}
        />
        <TextInput
          accessibilityLabel="Body weight in kg"
          autoCapitalize="none"
          autoCorrect={false}
          keyboardType="decimal-pad"
          onChangeText={(bodyWeightKg) => setForm((current) => ({ ...current, bodyWeightKg }))}
          placeholder="Body weight kg (optional)"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={form.bodyWeightKg}
        />
        <TextInput
          accessibilityLabel="Workout notes"
          autoCapitalize="none"
          autoCorrect={false}
          multiline
          onChangeText={(notes) => setForm((current) => ({ ...current, notes }))}
          placeholder="Notes (optional)"
          placeholderTextColor={colors.muted}
          style={styles.input}
          value={form.notes}
        />
        {form.entries.map((entry, entryIndex) => (
          <View key={`${entry.exerciseId}-${entryIndex}`} style={workoutStyles.entryCard}>
            <View style={shared.sectionHeader}>
              <Text style={styles.name}>{entry.exerciseName}</Text>
              <Pressable
                accessibilityLabel={`Remove ${entry.exerciseName}`}
                accessibilityRole="button"
                onPress={() =>
                  setForm((current) => ({
                    ...current,
                    entries: current.entries.filter((_, index) => index !== entryIndex),
                  }))
                }
                style={({ pressed }) => [workoutStyles.removeEntry, pressed && shared.iconPressed]}
              >
                <Text style={workoutStyles.removeEntryLabel}>Remove</Text>
              </Pressable>
            </View>
            {entry.sets.map((set, setIndex) => (
              <View key={setIndex} style={workoutStyles.setEditor}>
                <View style={shared.row}>
                  <TextInput
                    accessibilityLabel={`Reps for set ${setIndex + 1}`}
                    keyboardType="number-pad"
                    onChangeText={(reps) =>
                      setForm((current) => ({
                        ...current,
                        entries: current.entries.map((currentEntry, currentEntryIndex) =>
                          currentEntryIndex === entryIndex
                            ? {
                                ...currentEntry,
                                sets: currentEntry.sets.map((currentSet, currentSetIndex) =>
                                  currentSetIndex === setIndex
                                    ? { ...currentSet, reps }
                                    : currentSet,
                                ),
                              }
                            : currentEntry,
                        ),
                      }))
                    }
                    placeholder="Reps"
                    placeholderTextColor={colors.muted}
                    style={[styles.input, workoutStyles.setField]}
                    value={set.reps}
                  />
                  <TextInput
                    accessibilityLabel={`Added weight in kg for set ${setIndex + 1}`}
                    keyboardType="decimal-pad"
                    onChangeText={(weightKg) =>
                      setForm((current) => ({
                        ...current,
                        entries: current.entries.map((currentEntry, currentEntryIndex) =>
                          currentEntryIndex === entryIndex
                            ? {
                                ...currentEntry,
                                sets: currentEntry.sets.map((currentSet, currentSetIndex) =>
                                  currentSetIndex === setIndex
                                    ? { ...currentSet, weightKg }
                                    : currentSet,
                                ),
                              }
                            : currentEntry,
                        ),
                      }))
                    }
                    placeholder="kg"
                    placeholderTextColor={colors.muted}
                    style={[styles.input, workoutStyles.setField]}
                    value={set.weightKg}
                  />
                  <TextInput
                    accessibilityLabel={`Duration in seconds for set ${setIndex + 1}`}
                    keyboardType="number-pad"
                    onChangeText={(durationSeconds) =>
                      setForm((current) => ({
                        ...current,
                        entries: current.entries.map((currentEntry, currentEntryIndex) =>
                          currentEntryIndex === entryIndex
                            ? {
                                ...currentEntry,
                                sets: currentEntry.sets.map((currentSet, currentSetIndex) =>
                                  currentSetIndex === setIndex
                                    ? { ...currentSet, durationSeconds }
                                    : currentSet,
                                ),
                              }
                            : currentEntry,
                        ),
                      }))
                    }
                    placeholder="Secs"
                    placeholderTextColor={colors.muted}
                    style={[styles.input, workoutStyles.setField]}
                    value={set.durationSeconds}
                  />
                  <TextInput
                    accessibilityLabel={`RPE for set ${setIndex + 1}`}
                    keyboardType="decimal-pad"
                    onChangeText={(rpe) =>
                      setForm((current) => ({
                        ...current,
                        entries: current.entries.map((currentEntry, currentEntryIndex) =>
                          currentEntryIndex === entryIndex
                            ? {
                                ...currentEntry,
                                sets: currentEntry.sets.map((currentSet, currentSetIndex) =>
                                  currentSetIndex === setIndex
                                    ? { ...currentSet, rpe }
                                    : currentSet,
                                ),
                              }
                            : currentEntry,
                        ),
                      }))
                    }
                    placeholder="RPE"
                    placeholderTextColor={colors.muted}
                    style={[styles.input, workoutStyles.setField]}
                    value={set.rpe}
                  />
                </View>
                <TextInput
                  accessibilityLabel={`Notes for set ${setIndex + 1}`}
                  autoCapitalize="none"
                  autoCorrect={false}
                  onChangeText={(notes) =>
                    setForm((current) => ({
                      ...current,
                      entries: current.entries.map((currentEntry, currentEntryIndex) =>
                        currentEntryIndex === entryIndex
                          ? {
                              ...currentEntry,
                              sets: currentEntry.sets.map((currentSet, currentSetIndex) =>
                                currentSetIndex === setIndex
                                  ? { ...currentSet, notes }
                                  : currentSet,
                              ),
                            }
                          : currentEntry,
                      ),
                    }))
                  }
                  placeholder="Set notes (optional)"
                  placeholderTextColor={colors.muted}
                  style={styles.input}
                  value={set.notes}
                />
                {entry.sets.length > 1 ? (
                  <Pressable
                    accessibilityLabel={`Remove set ${setIndex + 1}`}
                    accessibilityRole="button"
                    onPress={() =>
                      setForm((current) => ({
                        ...current,
                        entries: current.entries.map((currentEntry, currentEntryIndex) =>
                          currentEntryIndex === entryIndex
                            ? {
                                ...currentEntry,
                                sets: currentEntry.sets.filter((_, index) => index !== setIndex),
                              }
                            : currentEntry,
                        ),
                      }))
                    }
                    style={({ pressed }) => [
                      workoutStyles.removeSet,
                      pressed && shared.iconPressed,
                    ]}
                  >
                    <Text style={workoutStyles.removeEntryLabel}>Remove set</Text>
                  </Pressable>
                ) : null}
              </View>
            ))}
            <Pressable
              accessibilityLabel={`Add set to ${entry.exerciseName}`}
              accessibilityRole="button"
              onPress={() =>
                setForm((current) => ({
                  ...current,
                  entries: current.entries.map((currentEntry, currentEntryIndex) =>
                    currentEntryIndex === entryIndex
                      ? { ...currentEntry, sets: [...currentEntry.sets, { ...emptySetRow }] }
                      : currentEntry,
                  ),
                }))
              }
              style={({ pressed }) => [workoutStyles.removeSet, pressed && shared.iconPressed]}
            >
              <Text style={workoutStyles.addSetLabel}>+ Add set</Text>
            </Pressable>
          </View>
        ))}
        <View style={shared.row}>
          <ActionButton
            label={editing ? "Save changes" : "Save workout"}
            sending={saving}
            disabled={saving}
            onPress={onSubmit}
          />
        </View>
        {error !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {error}
          </Text>
        ) : null}
      </View>
      <ExercisePicker
        onPick={(exercise) =>
          setForm((current) => ({
            ...current,
            entries: [
              ...current.entries,
              {
                exerciseId: exercise.id,
                exerciseName: exercise.name,
                notes: "",
                sets: [{ ...emptySetRow }],
              },
            ],
          }))
        }
        exercises={exercises}
      />
    </>
  );
}

/** A compact library picker so the log form can add exercises in order. */
function ExercisePicker({
  exercises,
  onPick,
}: {
  readonly exercises: readonly Exercise[];
  readonly onPick: (exercise: Exercise) => void;
}) {
  return (
    <View style={[shared.section, styles.card]}>
      <Text style={shared.sectionTitle}>ADD FROM LIBRARY</Text>
      {exercises.length === 0 ? (
        <Text style={styles.detail}>
          The exercise library is empty. Add exercises on the Library view.
        </Text>
      ) : (
        <View style={shared.row}>
          {exercises.map((exercise) => (
            <Pressable
              key={exercise.id}
              accessibilityLabel={`Add ${exercise.name} to the workout`}
              accessibilityRole="button"
              onPress={() => onPick(exercise)}
              style={({ pressed }) => [styles.chip, pressed && shared.actionPressed]}
            >
              <Text style={styles.chipLabel}>{exercise.name}</Text>
            </Pressable>
          ))}
        </View>
      )}
    </View>
  );
}

const workoutStyles = StyleSheet.create({
  addSetLabel: { color: colors.signal, fontSize: 13, fontWeight: "800" },
  entryCard: {
    backgroundColor: colors.panelRaised,
    borderColor: colors.border,
    borderRadius: 14,
    borderWidth: 1,
    gap: 10,
    padding: 12,
  },
  removeEntry: { minHeight: 32, justifyContent: "center", paddingHorizontal: 8 },
  removeEntryLabel: { color: colors.danger, fontSize: 13, fontWeight: "700" },
  removeSet: { minHeight: 32, justifyContent: "center" },
  setDetail: { color: colors.text, fontSize: 15, fontWeight: "600", lineHeight: 21 },
  setEditor: { gap: 8 },
  setField: { flex: 1, minHeight: 46, paddingHorizontal: 8 },
  setNotes: { color: colors.muted, fontSize: 13, lineHeight: 18 },
  setNumber: {
    color: colors.muted,
    fontSize: 12,
    fontWeight: "800",
    minWidth: 18,
    textAlign: "center",
  },
  setRow: {
    alignItems: "flex-start",
    flexDirection: "row",
    gap: 10,
  },
  workoutDate: { color: colors.text, fontSize: 17, fontWeight: "800", letterSpacing: -0.2 },
  workoutName: { color: colors.muted, fontSize: 15, fontWeight: "700", lineHeight: 21 },
  workoutOpen: { color: colors.signal, fontSize: 12, fontWeight: "800" },
});
