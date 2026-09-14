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

import { LineChart } from "react-native-gifted-charts";

import type { BodyActions, BodyState } from "../application/use-body";
import type { FitnessActions, FitnessState } from "../application/use-fitness";
import type { ProgressActions, ProgressState } from "../application/use-progress";
import type { WorkoutsActions, WorkoutsState } from "../application/use-workouts";
import {
  filterExercises,
  parseBodyWeightInput,
  parseExerciseInput,
  parseExerciseUpdate,
  parseProfileInput,
  parseWorkoutInput,
  type Exercise,
  type ExerciseCategory,
  type ProgressPoint,
  type SetInput,
  type WorkoutDetail,
  type WorkoutExerciseEntry,
  type WorkoutExerciseInput,
} from "../domain/fitness";
import { ActionButton, PageHeading, SecondaryAction, styles as shared } from "./shared";
import { colors } from "./theme";

type LibraryCategory = ExerciseCategory | "all";
/** Which of the Fitness Tab's four views is showing. */
type FitnessView = "library" | "workouts" | "progress" | "body";

type FormState = {
  readonly name: string;
  readonly category: ExerciseCategory;
  /** Untouched optional fields stay undefined so the edit PATCH keeps their current values. */
  readonly muscleGroup?: string;
  readonly equipment?: string;
};

const emptyForm: FormState = { name: "", category: "calisthenics" };

/** Prefill the shared form from an exercise, including its optional fields. */
function formFromExercise(exercise: Exercise): FormState {
  return {
    name: exercise.name,
    category: exercise.category as ExerciseCategory,
    muscleGroup: exercise.muscleGroup ?? "",
    equipment: exercise.equipment ?? "",
  };
}

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

function LibraryView({
  state,
  actions,
}: {
  readonly state: FitnessState;
  readonly actions: FitnessActions;
}) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<LibraryCategory>("all");
  const [muscleGroup, setMuscleGroup] = useState<string>("all");
  const [formOpen, setFormOpen] = useState(false);
  const [form, setForm] = useState<FormState>(emptyForm);
  // The exercise the shared form edits; null means the form creates.
  const [editing, setEditing] = useState<Exercise | null>(null);
  // The row whose edit/delete actions are revealed; null means collapsed.
  const [openId, setOpenId] = useState<number | null>(null);
  // The row whose delete button is armed; a second press confirms.
  const [confirmingId, setConfirmingId] = useState<number | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const wasBusy = useRef(false);
  const pending = useRef<
    { readonly kind: "create" } | { readonly kind: "update" } | { readonly kind: "delete"; readonly id: number } | null
  >(null);

  // Resolve the form once its own submission has succeeded; a failed
  // submission keeps the fields so only the failing part has to change.
  useEffect(() => {
    if (state.tag !== "ready") {
      return;
    }
    if (wasBusy.current && !state.busy && state.error === null && pending.current !== null) {
      const submitted = pending.current;
      pending.current = null;
      setFormError(null);
      if (submitted.kind === "create") {
        setForm(emptyForm);
      } else if (submitted.kind === "update") {
        setForm(emptyForm);
        setEditing(null);
      } else {
        setConfirmingId(null);
        setEditing((current) => (current?.id === submitted.id ? null : current));
      }
    }
    wasBusy.current = state.busy;
  }, [state]);

  const exercises = state.tag === "ready" ? state.exercises : [];
  // Distinct muscle groups across the library, for the filter chips.
  const muscleGroups = [
    ...new Set(
      exercises
        .map((exercise) => exercise.muscleGroup?.trim())
        .filter((group): group is string => group !== undefined && group.length > 0),
    ),
  ].sort((a, b) => a.localeCompare(b));
  const visible = filterExercises(
    exercises,
    query,
    category,
    muscleGroup === "all" ? "" : muscleGroup,
  );

  function submit(): void {
    if (editing !== null) {
      const parsed = parseExerciseUpdate(form.name, form.category, form.muscleGroup, form.equipment);
      if (!parsed.ok) {
        setFormError(parsed.error);
        return;
      }
      setFormError(null);
      pending.current = { kind: "update" };
      actions.update(editing.id, parsed.value);
      return;
    }
    const parsed = parseExerciseInput(
      form.name,
      form.category,
      form.muscleGroup ?? "",
      form.equipment ?? "",
    );
    if (!parsed.ok) {
      setFormError(parsed.error);
      return;
    }
    setFormError(null);
    pending.current = { kind: "create" };
    actions.create(parsed.value);
  }

  function openEdit(target: Exercise): void {
    setEditing(target);
    setForm(formFromExercise(target));
    setFormError(null);
    setFormOpen(false);
    setConfirmingId(null);
    setOpenId(null);
  }

  function cancelEdit(): void {
    setEditing(null);
    setForm(emptyForm);
    setFormError(null);
  }

  return (
    <>
      <View style={shared.stack}>
        <View style={[shared.section, styles.card]}>
          {editing === null ? (
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
          ) : (
            <Text style={shared.sectionTitle}>EDIT EXERCISE</Text>
          )}
          {editing !== null || formOpen ? (
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
                value={form.muscleGroup ?? ""}
              />
              <TextInput
                accessibilityLabel="Equipment"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(equipment) => setForm((current) => ({ ...current, equipment }))}
                placeholder="Equipment (optional)"
                placeholderTextColor={colors.muted}
                style={styles.input}
                value={form.equipment ?? ""}
              />
              <View style={shared.row}>
                {editing === null ? (
                  <ActionButton
                    label="Add exercise"
                    sending={state.tag === "ready" && state.busy}
                    disabled={state.tag !== "ready" || state.busy}
                    onPress={submit}
                  />
                ) : (
                  <>
                    <ActionButton
                      label="Save changes"
                      sending={state.tag === "ready" && state.busy}
                      disabled={state.tag !== "ready" || state.busy}
                      onPress={submit}
                    />
                    <SecondaryAction
                      label="Cancel"
                      disabled={state.tag !== "ready" || state.busy}
                      onPress={cancelEdit}
                    />
                  </>
                )}
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
        {muscleGroups.length > 0 ? (
          <View style={styles.chipWrap}>
            {[{ key: "all", label: "All" }, ...muscleGroups.map((group) => ({ key: group, label: group }))].map(
              ({ key, label }) => (
                <Pressable
                  key={key}
                  accessibilityLabel={`${label} muscle group filter`}
                  accessibilityRole="button"
                  accessibilityState={{ selected: muscleGroup === key }}
                  onPress={() => setMuscleGroup(key)}
                  style={({ pressed }) => [
                    styles.pickChip,
                    muscleGroup === key && styles.chipSelected,
                    pressed && shared.actionPressed,
                  ]}
                >
                  <Text style={[styles.chipLabel, muscleGroup === key && styles.chipLabelSelected]}>
                    {label}
                  </Text>
                </Pressable>
              ),
            )}
          </View>
        ) : null}

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
          ? groupByMuscleGroup(visible).map(([group, groupExercises]) => (
              <View key={group} style={shared.stack}>
                <Text style={styles.groupLabel}>{group.toUpperCase()}</Text>
                {groupExercises.map((exercise) => (
                  <ExerciseRow
                    key={exercise.id}
                    exercise={exercise}
                    busy={state.busy}
                    open={openId === exercise.id}
                    confirming={confirmingId === exercise.id}
                    onToggle={() =>
                      setOpenId((current) => (current === exercise.id ? null : exercise.id))
                    }
                    onEdit={() => openEdit(exercise)}
                    onDelete={() => {
                      if (confirmingId === exercise.id) {
                        pending.current = { kind: "delete", id: exercise.id };
                        actions.remove(exercise.id);
                      } else {
                        setConfirmingId(exercise.id);
                      }
                    }}
                  />
                ))}
              </View>
            ))
          : null}
        {state.tag === "ready" && state.error !== null && !formOpen && editing === null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.error}
          </Text>
        ) : null}
      </View>
    </>
  );
}

function ExerciseRow({
  exercise,
  busy,
  open,
  confirming,
  onToggle,
  onEdit,
  onDelete,
}: {
  readonly exercise: Exercise;
  readonly busy: boolean;
  readonly open: boolean;
  readonly confirming: boolean;
  readonly onToggle: () => void;
  readonly onEdit: () => void;
  readonly onDelete: () => void;
}) {
  return (
    <View style={shared.section}>
      <Pressable
        accessibilityLabel={`${open ? "Hide actions for" : "Show actions for"} ${exercise.name}`}
        accessibilityRole="button"
        accessibilityState={{ expanded: open }}
        onPress={onToggle}
        style={({ pressed }) => [styles.rowHeader, pressed && shared.iconPressed]}
      >
        <Text style={styles.name}>{exercise.name}</Text>
        <Text style={styles.rowChevron}>{open ? "Hide" : "Edit"}</Text>
      </Pressable>
      {open ? (
        <View style={styles.rowActions}>
          <Pressable
            accessibilityLabel={`Edit ${exercise.name}`}
            accessibilityRole="button"
            accessibilityState={{ disabled: busy }}
            disabled={busy}
            onPress={onEdit}
            style={({ pressed }) => [styles.rowAction, pressed && shared.iconPressed]}
          >
            <Text style={styles.rowActionLabel}>Edit</Text>
          </Pressable>
          <Pressable
            accessibilityLabel={
              confirming ? `Confirm deleting ${exercise.name}` : `Delete ${exercise.name}`
            }
            accessibilityRole="button"
            accessibilityState={{ disabled: busy }}
            disabled={busy}
            onPress={onDelete}
            style={({ pressed }) => [styles.rowAction, pressed && shared.iconPressed]}
          >
            <Text style={[styles.rowActionLabel, confirming && styles.rowActionConfirm]}>
              {confirming ? "Confirm delete?" : "Delete"}
            </Text>
          </Pressable>
        </View>
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
        <ActionButton label="Start workout" sending={false} disabled={false} onPress={onCreate} />
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
        <Text style={shared.retry}>‹ {editing ? "Back to workout" : "Discard workout"}</Text>
      </Pressable>
      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>{editing ? "EDIT WORKOUT" : "WORKOUT IN PROGRESS"}</Text>
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
            label={editing ? "Save changes" : "End workout"}
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
    </>
  );
}

/** Group exercises by muscle group for pickers; blank groups land in "Other". */
function groupByMuscleGroup(exercises: readonly Exercise[]): ReadonlyArray<[string, Exercise[]]> {
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

/** An inline library list grouped by muscle group so the workout form can add exercises in order. */
function ExercisePicker({
  exercises,
  onPick,
}: {
  readonly exercises: readonly Exercise[];
  readonly onPick: (exercise: Exercise) => void;
}) {
  return (
    <View style={shared.stack}>
      <Text style={shared.sectionTitle}>ADD EXERCISE</Text>
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
                  accessibilityLabel={`Add ${exercise.name} to the workout`}
                  accessibilityRole="button"
                  onPress={() => onPick(exercise)}
                  style={({ pressed }) => [styles.pickChip, pressed && shared.actionPressed]}
                >
                  <Text style={styles.chipLabel}>{exercise.name}</Text>
                </Pressable>
              ))}
            </View>
          </View>
        ))
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

/** Render the per-exercise progress view: an exercise picker and its trend series. */
function ProgressView({
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

type BodyWeightFormState = { readonly date: string; readonly weightKg: string };

/** Render the body weight log: record today's weight, past entries, and the profile. */
function BodyView({
  state,
  actions,
}: {
  readonly state: BodyState;
  readonly actions: BodyActions;
}) {
  const [form, setForm] = useState<BodyWeightFormState>({ date: today(), weightKg: "" });
  // Untouched profile fields stay undefined so the PATCH omits them and the
  // API keeps the current values; a cleared field becomes "" and sends null.
  const [profileForm, setProfileForm] = useState<{ heightCm?: string; sex?: string }>({});
  const [profileOpen, setProfileOpen] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [profileError, setProfileError] = useState<string | null>(null);
  const wasSaving = useRef(false);

  // Clear the weight form once its submission has succeeded; a failed save
  // keeps the fields so only the failing part has to change.
  useEffect(() => {
    if (wasSaving.current && !state.saving && state.saveError === null) {
      const latest = state.entries[0]?.weightKg;
      setForm({ date: today(), weightKg: latest === undefined ? "" : String(latest) });
      setFormError(null);
    }
    wasSaving.current = state.saving;
  }, [state]);

  function submitWeight(): void {
    const parsed = parseBodyWeightInput(form.date, form.weightKg);
    if (!parsed.ok) {
      setFormError(parsed.error);
      return;
    }
    setFormError(null);
    actions.saveWeight(parsed.value);
  }

  function submitProfile(): void {
    const parsed = parseProfileInput(profileForm.heightCm, profileForm.sex);
    if (!parsed.ok) {
      setProfileError(parsed.error);
      return;
    }
    setProfileError(null);
    actions.saveProfile(parsed.value);
  }

  const latestWeight = state.entries[0]?.weightKg;

  return (
    <>
      <View style={[shared.section, styles.card]}>
        <Pressable
          accessibilityLabel="Edit profile"
          accessibilityRole="button"
          accessibilityState={{ expanded: profileOpen }}
          onPress={() => setProfileOpen((value) => !value)}
          style={({ pressed }) => [styles.rowHeader, pressed && shared.iconPressed]}
        >
          <Text style={shared.sectionTitle}>PROFILE</Text>
          <Text style={styles.formToggle}>{profileOpen ? "Hide" : "Edit"}</Text>
        </Pressable>
        {state.profile.tag === "loaded" ? (
          <Text style={styles.detail}>
            {[
              state.profile.profile.heightCm !== null
                ? `${state.profile.profile.heightCm} cm`
                : null,
              state.profile.profile.sex !== null ? state.profile.profile.sex : null,
            ]
              .filter(Boolean)
              .join(" · ") || "Not configured"}
          </Text>
        ) : null}
        {state.profile.tag === "loading" ? (
          <Text style={styles.detail}>Loading profile</Text>
        ) : null}
        {state.profile.tag === "error" ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.profile.message}
          </Text>
        ) : null}
        {profileOpen ? (
          <>
            <View style={shared.row}>
              <TextInput
                accessibilityLabel="Height in cm"
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="decimal-pad"
                onChangeText={(heightCm) => setProfileForm((current) => ({ ...current, heightCm }))}
                placeholder="Height cm"
                placeholderTextColor={colors.muted}
                style={[styles.input, styles.halfInput]}
                value={profileForm.heightCm ?? ""}
              />
              <TextInput
                accessibilityLabel="Sex"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(sex) => setProfileForm((current) => ({ ...current, sex }))}
                placeholder="Sex, e.g. male"
                placeholderTextColor={colors.muted}
                style={[styles.input, styles.halfInput]}
                value={profileForm.sex ?? ""}
              />
            </View>
            <View style={shared.row}>
              <ActionButton
                label="Save profile"
                sending={state.saving}
                disabled={state.saving}
                onPress={submitProfile}
              />
            </View>
            {profileError !== null ? (
              <Text accessibilityRole="alert" style={shared.error}>
                {profileError}
              </Text>
            ) : null}
          </>
        ) : null}
      </View>

      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>RECORD WEIGHT</Text>
        <View style={shared.row}>
          <TextInput
            accessibilityLabel="Entry date"
            autoCapitalize="none"
            autoCorrect={false}
            onChangeText={(date) => setForm((current) => ({ ...current, date }))}
            placeholder="YYYY-MM-DD"
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={form.date}
          />
          <TextInput
            accessibilityLabel="Body weight in kg"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="decimal-pad"
            onChangeText={(weightKg) => setForm((current) => ({ ...current, weightKg }))}
            placeholder={latestWeight === undefined ? "Weight kg" : `Last: ${latestWeight} kg`}
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={form.weightKg}
          />
        </View>
        <View style={shared.row}>
          <ActionButton
            label="Record weight"
            sending={state.saving}
            disabled={state.saving}
            onPress={submitWeight}
          />
        </View>
        {formError !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {formError}
          </Text>
        ) : null}
        {state.saveError !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.saveError}
          </Text>
        ) : null}
      </View>

      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>PROFILE</Text>
        {state.profile.tag === "loaded" &&
        (state.profile.profile.heightCm !== null || state.profile.profile.sex !== null) ? (
          <Text style={styles.detail}>
            Current:{" "}
            {[
              state.profile.profile.heightCm !== null
                ? `${state.profile.profile.heightCm} cm`
                : null,
              state.profile.profile.sex !== null ? state.profile.profile.sex : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </Text>
        ) : null}
        {state.profile.tag === "loading" ? (
          <Text style={styles.detail}>Loading profile</Text>
        ) : null}
        {state.profile.tag === "error" ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.profile.message}
          </Text>
        ) : null}
        <View style={shared.row}>
          <TextInput
            accessibilityLabel="Height in cm"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="decimal-pad"
            onChangeText={(heightCm) => setProfileForm((current) => ({ ...current, heightCm }))}
            placeholder="Height cm"
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={profileForm.heightCm ?? ""}
          />
          <TextInput
            accessibilityLabel="Sex"
            autoCapitalize="none"
            autoCorrect={false}
            onChangeText={(sex) => setProfileForm((current) => ({ ...current, sex }))}
            placeholder="Sex, e.g. male"
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={profileForm.sex ?? ""}
          />
        </View>
        <View style={shared.row}>
          <ActionButton
            label="Save profile"
            sending={state.saving}
            disabled={state.saving}
            onPress={submitProfile}
          />
        </View>
        {profileError !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {profileError}
          </Text>
        ) : null}
      </View>

      {state.refreshing ? (
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Refreshing</Text>
        </View>
      ) : null}
      {state.entries.length === 0 && !state.refreshing ? (
        <View style={shared.statusPanel}>
          <Text style={shared.statusText}>
            {state.error === null ? "No body weight entries yet" : state.error}
          </Text>
        </View>
      ) : null}
      {state.entries.map((entry) => (
        <View key={entry.id} style={shared.section}>
          <View style={shared.sectionHeader}>
            <Text style={styles.name}>{formatDate(entry.date)}</Text>
            <Text style={styles.detail}>{entry.weightKg} kg</Text>
          </View>
        </View>
      ))}
      <Pressable
        accessibilityRole="button"
        onPress={actions.refresh}
        style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
      >
        <Text style={shared.retry}>Refresh entries</Text>
      </Pressable>
    </>
  );
}
