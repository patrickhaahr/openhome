import { useId, useState } from "react"
import { z } from "zod"
import { CheckIcon } from "@phosphor-icons/react/dist/csr/Check"
import { ScalesIcon } from "@phosphor-icons/react/dist/csr/Scales"
import { useIdleBlock } from "@/components/idle-block"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import {
  optionalNumber,
  profileDraft,
  profilePatch,
  profileSexes,
  type BodyCommand,
} from "@/lib/body-profile"
import { trainingDayOn } from "@/lib/workout"

/** A real `YYYY-MM-DD` day no later than today in Copenhagen. */
function isPastDay(value: string): boolean {
  return (
    z.iso.date().safeParse(value).success && value <= trainingDayOn(new Date())
  )
}

export type ProfileEditing = {
  readonly original: ReturnType<typeof profileDraft>
  readonly draft: ReturnType<typeof profileDraft>
}

type ProfileEditorProps = {
  readonly original: ProfileEditing["original"]
  readonly draft: ProfileEditing["draft"]
  readonly update: (draft: ProfileEditing["draft"]) => void
  readonly disabled: boolean
  readonly save: (command: BodyCommand) => Promise<boolean>
  readonly cancel: () => void
  readonly failure: string | undefined
}

export function ProfileEditor({
  original,
  draft,
  update,
  disabled,
  save,
  cancel,
  failure,
}: ProfileEditorProps): React.JSX.Element {
  const [problem, setProblem] = useState<{
    field: "height" | "heart" | "birth"
    message: string
  }>()

  const id = useId()

  // A sex stored by another client that the targets cannot use stays selectable until changed.
  const unsupportedSex =
    original.sex === "" ||
    profileSexes.some((sex) => sex.value === original.sex)
      ? undefined
      : original.sex

  return (
    <form
      className="workout-editor"
      aria-label="Edit profile"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        const patch = profilePatch(original, draft)

        if (
          patch.height_cm !== undefined &&
          patch.height_cm !== null &&
          (!Number.isFinite(patch.height_cm) || patch.height_cm <= 0)
        ) {
          setProblem({
            field: "height",
            message:
              "Enter height in cm above zero, or leave it empty to clear.",
          })
          document.getElementById(`${id}-height`)?.focus()

          return
        }

        if (
          patch.hr_max_bpm !== undefined &&
          patch.hr_max_bpm !== null &&
          (!Number.isInteger(patch.hr_max_bpm) ||
            patch.hr_max_bpm < 100 ||
            patch.hr_max_bpm > 240)
        ) {
          setProblem({
            field: "heart",
            message:
              "Enter a whole maximum heart rate from 100 to 240 bpm, or leave it empty to clear.",
          })
          document.getElementById(`${id}-heart`)?.focus()

          return
        }

        if (
          patch.birth_date !== undefined &&
          patch.birth_date !== null &&
          !isPastDay(patch.birth_date)
        ) {
          setProblem({
            field: "birth",
            message:
              "Enter a birth date written YYYY-MM-DD that is not in the future, or leave it empty to clear.",
          })
          document.getElementById(`${id}-birth`)?.focus()

          return
        }

        setProblem(undefined)
        void save(patch)
      }}
    >
      <p className="sr-only">
        Leave a field empty to clear it. Unchanged fields keep their stored
        values.
      </p>
      <div className="workout-field">
        <label htmlFor={`${id}-height`}>Height (cm)</label>
        <Input
          id={`${id}-height`}
          aria-invalid={problem?.field === "height"}
          aria-describedby={
            problem?.field === "height" ? `${id}-problem` : undefined
          }
          inputMode="decimal"
          value={draft.height}
          disabled={disabled}
          onChange={(event) => {
            update({ ...draft, height: event.target.value })
            setProblem(undefined)
          }}
        />
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-sex`}>Sex (optional)</label>
        <NativeSelect
          id={`${id}-sex`}
          value={draft.sex}
          disabled={disabled}
          onChange={(event) => update({ ...draft, sex: event.target.value })}
        >
          <NativeSelectOption value="">Not set</NativeSelectOption>
          {profileSexes.map((sex) => (
            <NativeSelectOption key={sex.value} value={sex.value}>
              {sex.label}
            </NativeSelectOption>
          ))}
          {unsupportedSex === undefined ? null : (
            <NativeSelectOption value={unsupportedSex}>
              {`“${unsupportedSex}” (not used for targets)`}
            </NativeSelectOption>
          )}
        </NativeSelect>
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-heart`}>Maximum heart rate (bpm)</label>
        <Input
          id={`${id}-heart`}
          aria-invalid={problem?.field === "heart"}
          aria-describedby={
            problem?.field === "heart" ? `${id}-problem` : undefined
          }
          inputMode="numeric"
          value={draft.heartRate}
          disabled={disabled}
          onChange={(event) => {
            update({ ...draft, heartRate: event.target.value })
            setProblem(undefined)
          }}
        />
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-birth`}>Birth date</label>
        <Input
          id={`${id}-birth`}
          type="date"
          aria-invalid={problem?.field === "birth"}
          aria-describedby={
            problem?.field === "birth" ? `${id}-problem` : undefined
          }
          value={draft.birthDate}
          disabled={disabled}
          onChange={(event) => {
            update({ ...draft, birthDate: event.target.value })
            setProblem(undefined)
          }}
        />
      </div>
      {problem !== undefined || failure !== undefined ? (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem?.message ?? failure}
        </p>
      ) : null}
      <div className="workout-editor-actions">
        <Button type="submit" disabled={disabled}>
          Save profile
        </Button>
        <Button variant="outline" disabled={disabled} onClick={cancel}>
          Cancel profile edit
        </Button>
      </div>
    </form>
  )
}

export function WeightEditor({
  disabled,
  save,
}: {
  readonly disabled: boolean
  readonly save: (command: BodyCommand) => Promise<boolean>
}): React.JSX.Element {
  const [date, setDate] = useState(() => trainingDayOn(new Date()))
  const [weight, setWeight] = useState("")
  // A typed weight is a draft.
  useIdleBlock(weight !== "")

  const [problem, setProblem] = useState<{
    field: "date" | "weight"
    message: string
  }>()

  const id = useId()

  return (
    <form
      className="body-weight-form"
      aria-label="Log body weight"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()

        if (!z.iso.date().safeParse(date).success) {
          setProblem({
            field: "date",
            message: "Choose a valid calendar date.",
          })
          document.getElementById(`${id}-date`)?.focus()

          return
        }

        const value = optionalNumber(weight)

        if (value === null || !Number.isFinite(value) || value <= 0) {
          setProblem({
            field: "weight",
            message: "Enter body weight in kg above zero.",
          })
          document.getElementById(`${id}-weight`)?.focus()

          return
        }

        setProblem(undefined)
        void save({ kind: "weight", date, weight_kg: value }).then((saved) => {
          if (saved) setWeight("")
        })
      }}
    >
      <div className="body-weight-fields">
        <div className="body-weight-amount">
          <label htmlFor={`${id}-weight`} className="sr-only">
            Body weight (kg)
          </label>
          <ScalesIcon size={24} aria-hidden="true" />
          <Input
            id={`${id}-weight`}
            aria-invalid={problem?.field === "weight"}
            aria-describedby={
              problem?.field === "weight" ? `${id}-problem` : undefined
            }
            inputMode="decimal"
            placeholder="00.0"
            value={weight}
            disabled={disabled}
            onChange={(event) => {
              setWeight(event.target.value)
              setProblem(undefined)
            }}
          />
          <span aria-hidden="true">kg</span>
        </div>
        <div className="body-weight-date">
          <label htmlFor={`${id}-date`} className="sr-only">
            Weight date
          </label>
          <Input
            id={`${id}-date`}
            aria-invalid={problem?.field === "date"}
            aria-describedby={
              problem?.field === "date" ? `${id}-problem` : undefined
            }
            type="date"
            value={date}
            disabled={disabled}
            onChange={(event) => {
              setDate(event.target.value)
              setProblem(undefined)
            }}
          />
        </div>
        <Button type="submit" size="key-icon" disabled={disabled}>
          <CheckIcon size={22} weight="bold" aria-hidden="true" />
          <span className="sr-only">Save body weight</span>
        </Button>
      </div>
      {problem === undefined ? null : (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem.message}
        </p>
      )}
    </form>
  )
}
