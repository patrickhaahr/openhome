import { ActivityIndicator, Pressable, Text, TextInput, View } from "react-native";
import { useEffect, useRef, useState } from "react";

import type { FitnessActions, FitnessState } from "../../application/use-fitness";
import {
  filterExercises,
  parseExerciseInput,
  parseExerciseUpdate,
  type Exercise,
  type ExerciseCategory,
} from "../../domain/fitness";
import { ActionButton, SecondaryAction, styles as shared } from "../shared";
import { colors } from "../theme";
import { groupByMuscleGroup, styles } from "./shared";

type LibraryCategory = ExerciseCategory | "all";

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

export function LibraryView({
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
