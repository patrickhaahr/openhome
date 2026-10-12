import { useId, useRef, useState } from "react"
import { ArrowDownIcon } from "@phosphor-icons/react/dist/csr/ArrowDown"
import { ArrowUpIcon } from "@phosphor-icons/react/dist/csr/ArrowUp"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { ExercisePicker } from "@/components/exercise-picker"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import { Textarea } from "@/components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { createRequestId } from "@/lib/request-id"
import {
  nextSet,
  parseDraft,
  type DraftField,
  type DraftProblems,
  type EntryDraft,
  type Exercise,
  type SetDraft,
  type WorkoutDraft,
  type WorkoutInput,
} from "@/lib/workout"

type WorkoutEditorProps = {
  readonly title: string
  readonly submitLabel: string
  readonly initial: WorkoutDraft
  readonly exercises: ReadonlyArray<Exercise>
  readonly disabled: boolean
  readonly save: (input: WorkoutInput) => Promise<boolean>
  readonly cancel: () => void
}

const rpeValues = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"] as const

/** Compose or edit a whole Workout; the draft survives failed saves until it is cancelled. */
export function WorkoutEditor({
  title,
  submitLabel,
  initial,
  exercises,
  disabled,
  save,
  cancel,
}: WorkoutEditorProps): React.JSX.Element {
  const [draft, setDraft] = useState(initial)
  const [problems, setProblems] = useState<DraftProblems>(new Map())
  const form = useRef<HTMLFormElement>(null)
  const id = useId()

  function updateEntry(
    key: string,
    change: (entry: EntryDraft) => EntryDraft
  ): void {
    setDraft((current) => ({
      ...current,
      entries: current.entries.map((entry) =>
        entry.key === key ? change(entry) : entry
      ),
    }))
  }

  function updateSet(
    entryKey: string,
    setKey: string,
    change: Partial<Omit<SetDraft, "key" | "notes">>
  ): void {
    updateEntry(entryKey, (entry) => ({
      ...entry,
      sets: entry.sets.map((set) =>
        set.key === setKey ? { ...set, ...change } : set
      ),
    }))

    if (problems.has(setKey)) {
      const remaining = new Map(problems)
      remaining.delete(setKey)
      setProblems(remaining)
    }
  }

  function moveEntry(index: number, offset: -1 | 1): void {
    setDraft((current) => {
      const entries = [...current.entries]
      const [moved] = entries.splice(index, 1)

      if (moved !== undefined) entries.splice(index + offset, 0, moved)

      return { ...current, entries }
    })
  }

  return (
    <form
      ref={form}
      className="workout-editor"
      aria-label={title}
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        const parsed = parseDraft(draft)

        if (parsed.state === "problems") {
          setProblems(parsed.problems)
          // Move touch users to the first field that needs attention.
          requestAnimationFrame(() =>
            form.current
              ?.querySelector<HTMLElement>("[aria-invalid='true']")
              ?.focus()
          )

          return
        }

        setProblems(new Map())
        void save(parsed.input)
      }}
    >
      <h2>{title}</h2>
      <div className="workout-editor-fields">
        <div className="workout-field">
          <label htmlFor={`${id}-date`}>Date</label>
          <Input
            id={`${id}-date`}
            type="date"
            required
            value={draft.date}
            aria-invalid={problems.has("date")}
            aria-describedby={
              problems.has("date") ? `${id}-date-problem` : undefined
            }
            disabled={disabled}
            onChange={(event) =>
              setDraft({ ...draft, date: event.target.value })
            }
          />
          {problems.has("date") ? (
            <p id={`${id}-date-problem`} className="workout-problem">
              {problems.get("date")?.message}
            </p>
          ) : null}
        </div>
        <div className="workout-field">
          <label htmlFor={`${id}-name`}>Name (optional)</label>
          <Input
            id={`${id}-name`}
            value={draft.name}
            placeholder="Pull day"
            autoComplete="off"
            disabled={disabled}
            onChange={(event) =>
              setDraft({ ...draft, name: event.target.value })
            }
          />
        </div>
      </div>
      {draft.entries.length === 0 ? (
        <p className="workout-muted">
          No exercises yet. Add one from the library to log its sets.
        </p>
      ) : null}
      <ol className="workout-entries" aria-label="Exercises in this workout">
        {draft.entries.map((entry, index) => (
          <li key={entry.key} className="workout-entry-editor">
            <div className="workout-entry-heading">
              <span className="workout-entry-number" aria-hidden="true">
                {String(index + 1).padStart(2, "0")}
              </span>
              <h3>{entry.exercise.name}</h3>
              <div className="workout-entry-actions">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-lg"
                  aria-label={`Move ${entry.exercise.name} up`}
                  disabled={disabled || index === 0}
                  onClick={() => moveEntry(index, -1)}
                >
                  <ArrowUpIcon aria-hidden="true" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-lg"
                  aria-label={`Move ${entry.exercise.name} down`}
                  disabled={disabled || index === draft.entries.length - 1}
                  onClick={() => moveEntry(index, 1)}
                >
                  <ArrowDownIcon aria-hidden="true" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-lg"
                  aria-label={`Remove ${entry.exercise.name} from this workout`}
                  disabled={disabled}
                  onClick={() =>
                    setDraft({
                      ...draft,
                      entries: draft.entries.filter(
                        (item) => item.key !== entry.key
                      ),
                    })
                  }
                >
                  <TrashIcon aria-hidden="true" />
                </Button>
              </div>
            </div>
            <ol
              className="workout-set-rows"
              aria-label={`${entry.exercise.name} sets`}
            >
              {entry.sets.map((set, setIndex) => {
                const label = `${entry.exercise.name} set ${setIndex + 1}`
                const problem = problems.get(set.key)
                const problemId = `${id}-${set.key}-problem`

                const invalid = (field: DraftField): boolean =>
                  problem?.field === field

                return (
                  <li key={set.key}>
                    <div className="workout-set-row workout-choice">
                      <span className="workout-set-number">
                        Set {setIndex + 1}
                      </span>
                      <ToggleGroup
                        aria-label={`${label} type`}
                        variant="outline"
                        spacing={0}
                        value={[set.kind]}
                        disabled={disabled}
                        onValueChange={([next]) => {
                          if (next === "reps" || next === "hold")
                            updateSet(entry.key, set.key, { kind: next })
                        }}
                      >
                        <ToggleGroupItem value="reps" size="lg">
                          Reps
                        </ToggleGroupItem>
                        <ToggleGroupItem value="hold" size="lg">
                          Hold
                        </ToggleGroupItem>
                      </ToggleGroup>
                      {set.kind === "reps" ? (
                        <Input
                          aria-label={`${label} reps`}
                          placeholder="Reps"
                          inputMode="numeric"
                          autoComplete="off"
                          value={set.reps}
                          aria-invalid={invalid("reps")}
                          aria-describedby={
                            invalid("reps") ? problemId : undefined
                          }
                          disabled={disabled}
                          onChange={(event) =>
                            updateSet(entry.key, set.key, {
                              reps: event.target.value,
                            })
                          }
                        />
                      ) : (
                        <Input
                          aria-label={`${label} hold time`}
                          placeholder="Seconds or m:ss"
                          inputMode="numeric"
                          autoComplete="off"
                          value={set.seconds}
                          aria-invalid={invalid("hold")}
                          aria-describedby={
                            invalid("hold") ? problemId : undefined
                          }
                          disabled={disabled}
                          onChange={(event) =>
                            updateSet(entry.key, set.key, {
                              seconds: event.target.value,
                            })
                          }
                        />
                      )}
                      <div className="workout-unit">
                        <Input
                          aria-label={`${label} added weight in kg`}
                          placeholder="Bodyweight"
                          inputMode="decimal"
                          autoComplete="off"
                          value={set.weight}
                          aria-invalid={invalid("weight")}
                          aria-describedby={
                            invalid("weight") ? problemId : undefined
                          }
                          disabled={disabled}
                          onChange={(event) =>
                            updateSet(entry.key, set.key, {
                              weight: event.target.value,
                            })
                          }
                        />
                        <span aria-hidden="true">kg</span>
                      </div>
                      <NativeSelect
                        aria-label={`${label} RPE`}
                        value={set.rpe}
                        aria-invalid={invalid("rpe")}
                        aria-describedby={
                          invalid("rpe") ? problemId : undefined
                        }
                        disabled={disabled}
                        onChange={(event) =>
                          updateSet(entry.key, set.key, {
                            rpe: event.target.value,
                          })
                        }
                      >
                        <NativeSelectOption value="">No RPE</NativeSelectOption>
                        {rpeValues.map((value) => (
                          <NativeSelectOption key={value} value={value}>
                            RPE {value}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-lg"
                        aria-label={`Remove ${label}`}
                        disabled={disabled}
                        onClick={() =>
                          updateEntry(entry.key, (current) => ({
                            ...current,
                            sets: current.sets.filter(
                              (item) => item.key !== set.key
                            ),
                          }))
                        }
                      >
                        <XIcon aria-hidden="true" />
                      </Button>
                    </div>
                    {problem === undefined ? null : (
                      <p id={problemId} className="workout-problem">
                        {problem.message}
                      </p>
                    )}
                    {set.notes === null ? null : (
                      <p className="workout-muted">Note: {set.notes}</p>
                    )}
                  </li>
                )
              })}
            </ol>
            <div className="workout-entry-footer">
              <Button
                type="button"
                variant="ghost"
                disabled={disabled}
                onClick={() =>
                  updateEntry(entry.key, (current) => ({
                    ...current,
                    sets: [
                      ...current.sets,
                      nextSet(
                        createRequestId(),
                        current.sets[current.sets.length - 1]
                      ),
                    ],
                  }))
                }
              >
                <PlusIcon aria-hidden="true" />
                Add set
              </Button>
              <Input
                aria-label={`${entry.exercise.name} notes`}
                placeholder="Exercise notes (optional)"
                autoComplete="off"
                value={entry.notes}
                disabled={disabled}
                onChange={(event) =>
                  updateEntry(entry.key, (current) => ({
                    ...current,
                    notes: event.target.value,
                  }))
                }
              />
            </div>
          </li>
        ))}
      </ol>
      <ExercisePicker
        exercises={exercises}
        disabled={disabled}
        choose={(exercise) =>
          setDraft({
            ...draft,
            entries: [
              ...draft.entries,
              {
                key: createRequestId(),
                exercise,
                notes: "",
                sets: [nextSet(createRequestId(), undefined)],
              },
            ],
          })
        }
      />
      <div className="workout-field">
        <label htmlFor={`${id}-notes`}>Workout notes (optional)</label>
        <Textarea
          id={`${id}-notes`}
          value={draft.notes}
          disabled={disabled}
          onChange={(event) =>
            setDraft({ ...draft, notes: event.target.value })
          }
        />
      </div>
      {problems.size > 0 ? (
        <p className="workout-problem" role="alert">
          Check the highlighted fields. Nothing was saved.
        </p>
      ) : null}
      <div className="workout-editor-actions">
        <Button type="submit" size="lg" disabled={disabled}>
          {submitLabel}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="lg"
          disabled={disabled}
          onClick={cancel}
        >
          Cancel
        </Button>
      </div>
    </form>
  )
}
