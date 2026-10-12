import { useId, useState } from "react"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { Exercise } from "@/lib/workout"

type CategoryFilter = Exercise["category"] | "all"

const categoryLabels: Record<CategoryFilter, string> = {
  all: "All",
  calisthenics: "Calisthenics",
  gym: "Gym",
}

type ExercisePickerProps = {
  readonly exercises: ReadonlyArray<Exercise>
  readonly disabled: boolean
  readonly choose: (exercise: Exercise) => void
}

/** Browse the shared exercise library by name, category and muscle group, then add one entry. */
export function ExercisePicker({
  exercises,
  disabled,
  choose,
}: ExercisePickerProps): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const [category, setCategory] = useState<CategoryFilter>("all")
  const searchId = useId()
  const needle = query.trim().toLocaleLowerCase()

  const matches = exercises.filter(
    (exercise) =>
      (category === "all" || exercise.category === category) &&
      (needle === "" ||
        [exercise.name, exercise.muscle_group, exercise.equipment].some(
          (text) => text?.toLocaleLowerCase().includes(needle) === true
        ))
  )

  // Map.groupBy needs Safari 17.4; the household iPad may be older.
  const groups = new Map<string, Exercise[]>()

  for (const exercise of matches) {
    const group = exercise.muscle_group ?? "Other"
    const members = groups.get(group)

    if (members === undefined) groups.set(group, [exercise])
    else members.push(exercise)
  }

  return (
    <>
      <Button
        type="button"
        variant="outline"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <PlusIcon aria-hidden="true" />
        Add exercise
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>Add an exercise</DialogTitle>
            <DialogDescription>
              Choose from the shared exercise library.
            </DialogDescription>
          </DialogHeader>
          <div className="exercise-picker-filters workout-choice">
            <label htmlFor={searchId} className="sr-only">
              Search exercises
            </label>
            <Input
              id={searchId}
              type="search"
              placeholder="Search name, muscle or equipment"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              autoComplete="off"
            />
            <ToggleGroup
              aria-label="Exercise category"
              variant="outline"
              spacing={0}
              value={[category]}
              onValueChange={([next]) => {
                if (next === "all" || next === "calisthenics" || next === "gym")
                  setCategory(next)
              }}
            >
              {(["all", "calisthenics", "gym"] as const).map((value) => (
                <ToggleGroupItem key={value} value={value} size="lg">
                  {categoryLabels[value]}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </div>
          <div className="exercise-picker-results">
            {matches.length === 0 ? (
              <p className="workout-muted">
                {exercises.length === 0
                  ? "The exercise library is empty."
                  : "No exercises match. Clear the search or choose All."}
              </p>
            ) : null}
            {Array.from(groups, ([group, members]) => (
              <section key={group} aria-label={group}>
                <h3>{group}</h3>
                <ul>
                  {members.map((exercise) => (
                    <li key={exercise.id}>
                      <Button
                        type="button"
                        variant="ghost"
                        aria-label={`Add ${exercise.name}`}
                        onClick={() => {
                          choose(exercise)
                          setOpen(false)
                        }}
                      >
                        <span>{exercise.name}</span>
                        <span className="workout-muted">
                          {[
                            categoryLabels[exercise.category],
                            exercise.equipment,
                          ]
                            .filter((part) => part !== null)
                            .join(" · ")}
                        </span>
                      </Button>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
