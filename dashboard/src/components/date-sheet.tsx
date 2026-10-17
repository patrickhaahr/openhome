import { useId, useRef, useState } from "react"
import { HourglassSimpleIcon } from "@phosphor-icons/react/dist/csr/HourglassSimple"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { RepeatIcon } from "@phosphor-icons/react/dist/csr/Repeat"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { PersonDisc, Segments } from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  importantDateCommand,
  figureParts,
  milestoneFigure,
  type ImportantDate,
  type ImportantDateCommand,
} from "@/lib/important-dates"
import { formatDay, trimText } from "@/lib/lists"
import { createRequestId } from "@/lib/request-id"
import type { HouseholdUser } from "@/lib/wishlist"

type Save = (command: ImportantDateCommand, saved: string) => Promise<boolean>

/** A quiet mark for how a date counts; the word is kept for assistive technology. */
export function DateKindGlyph({
  kind,
  size = 18,
}: {
  kind: ImportantDate["kind"]
  size?: number
}): React.JSX.Element {
  return (
    <span className="kind-glyph">
      {kind === "since" ? (
        <HourglassSimpleIcon size={size} aria-hidden="true" />
      ) : (
        <RepeatIcon size={size} aria-hidden="true" />
      )}
      <span className="sr-only">{kind === "since" ? "Since" : "Yearly"}</span>
    </span>
  )
}

/** Milestones lead the Dates view; ordinary figures wrap only between their elapsed parts. */
export function DateFigure({
  date,
  large = false,
}: {
  date: ImportantDate
  large?: boolean
}): React.JSX.Element {
  const milestone = milestoneFigure(date)
  const parts = milestone === undefined ? figureParts(date) : [milestone]

  return (
    <span
      className={
        large ? "date-figure date-figure-large num" : "date-figure num"
      }
      data-milestone={date.milestone !== null}
    >
      {parts.map((part, index) => (
        <span key={part} className="date-figure-part">
          {index === 0 ? null : <span aria-hidden="true">·</span>}
          {index === 0 ? null : <span className="sr-only">, </span>}
          {part}
        </span>
      ))}
    </span>
  )
}

/** One switch row; the whole row is the key. */
export function IdleSwitch({
  checked,
  disabled,
  toggle,
}: {
  checked: boolean
  disabled: boolean
  toggle: (next: boolean) => void
}): React.JSX.Element {
  return (
    <button
      type="button"
      role="switch"
      className="idle-switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => toggle(!checked)}
    >
      <span className="idle-switch-text">
        Show when idle
        <span className="idle-switch-hint">
          On the photo screen, for anyone in the room
        </span>
      </span>
      <span className="idle-switch-track" aria-hidden="true">
        <span className="idle-switch-thumb" />
      </span>
    </button>
  )
}

const kinds = [
  {
    value: "since",
    label: "Since",
    icon: <HourglassSimpleIcon size={18} aria-hidden="true" />,
  },
  {
    value: "yearly",
    label: "Yearly",
    icon: <RepeatIcon size={18} aria-hidden="true" />,
  },
] as const satisfies ReadonlyArray<{
  value: ImportantDate["kind"]
  label: string
  icon: React.ReactNode
}>

/** The editable part of a date; tags are always an explicit set. */
export type DateFields = {
  title: string
  date: string
  kind: ImportantDate["kind"]
  person_ids: string[]
  show_when_idle: boolean
}

/** Title, date, how it counts, who it is about and whether the idle screen shows it. */
function DateForm({
  owners,
  initial,
  busy,
  submit,
  label,
  cancel,
}: {
  owners: readonly HouseholdUser[]
  initial: DateFields
  busy: boolean
  submit: (fields: DateFields) => void
  label: string
  cancel?: () => void
}): React.JSX.Element {
  const id = useId()
  const [fields, setFields] = useState(initial)
  const title = trimText(fields.title)

  const set = (patch: Partial<DateFields>) =>
    setFields((previous) => ({ ...previous, ...patch }))

  return (
    <form
      className="sheet-body date-form"
      onSubmit={(event) => {
        event.preventDefault()

        if (title !== "" && fields.date !== "") submit({ ...fields, title })
      }}
    >
      <label className="field field-title" htmlFor={`${id}-title`}>
        <span>Title</span>
        <Input
          id={`${id}-title`}
          value={fields.title}
          required
          maxLength={200}
          autoComplete="off"
          placeholder="Together"
          disabled={busy}
          onChange={(event) => set({ title: event.target.value })}
        />
      </label>
      <label className="field" htmlFor={`${id}-date`}>
        <span>Date</span>
        <Input
          id={`${id}-date`}
          type="date"
          value={fields.date}
          required
          disabled={busy}
          onChange={(event) => set({ date: event.target.value })}
        />
      </label>
      <div className="field">
        <span>Counts</span>
        <Segments
          label="Counts"
          value={fields.kind}
          options={kinds}
          disabled={busy}
          choose={(kind) => set({ kind })}
        />
        <span className="field-hint">
          {fields.kind === "since"
            ? "Counts up: years, months and days since."
            : "Counts down to the next anniversary."}
        </span>
      </div>
      {owners.length === 0 ? null : (
        <fieldset className="field">
          <legend>Who it is about</legend>
          <div className="date-people">
            {owners.map((owner) => {
              const tagged = fields.person_ids.includes(owner.id)

              return (
                <button
                  key={owner.id}
                  type="button"
                  className="view-tab person-key"
                  aria-pressed={tagged}
                  disabled={busy}
                  onClick={() =>
                    set({
                      person_ids: tagged
                        ? fields.person_ids.filter((id) => id !== owner.id)
                        : [...fields.person_ids, owner.id],
                    })
                  }
                >
                  <PersonDisc name={owner.name} />
                  {owner.name}
                </button>
              )
            })}
          </div>
          <span className="field-hint">
            {fields.person_ids.length === 0
              ? "No one: it shows for everyone."
              : "It shows for them, and under Everyone."}
          </span>
        </fieldset>
      )}
      <IdleSwitch
        checked={fields.show_when_idle}
        disabled={busy}
        toggle={(show_when_idle) => set({ show_when_idle })}
      />
      <div className="sheet-actions">
        <Button
          type="submit"
          size="key"
          disabled={busy || title === "" || fields.date === ""}
        >
          {label}
        </Button>
        {cancel === undefined ? null : (
          <Button
            type="button"
            variant="ghost"
            size="key"
            disabled={busy}
            onClick={cancel}
          >
            Cancel
          </Button>
        )}
      </div>
    </form>
  )
}

type Addition = Extract<ImportantDateCommand, { action: "add" }>

const pendingKey = "openhome.pending-date"

function readPending(): Addition | undefined {
  try {
    const parsed = importantDateCommand.safeParse(
      JSON.parse(sessionStorage.getItem(pendingKey) ?? "null")
    )

    if (parsed.success && parsed.data.action === "add") return parsed.data
  } catch {
    /* Private browsing may disable storage. */
  }

  return undefined
}

function storePending(addition: Addition | undefined): void {
  try {
    if (addition) sessionStorage.setItem(pendingKey, JSON.stringify(addition))
    else sessionStorage.removeItem(pendingKey)
  } catch {
    /* The mounted flow keeps its in-memory retry identity. */
  }
}

function sameFields(a: DateFields, b: DateFields): boolean {
  return (
    a.title === b.title &&
    a.date === b.date &&
    a.kind === b.kind &&
    a.show_when_idle === b.show_when_idle &&
    a.person_ids.length === b.person_ids.length &&
    a.person_ids.every((id) => b.person_ids.includes(id))
  )
}

/** Add a date; a lost response retried with the same fields keeps its request ID. */
export function DateAdd({
  owners,
  person,
  busy,
  save,
  done,
}: {
  owners: readonly HouseholdUser[]
  person: string | undefined
  busy: boolean
  save: Save
  done: () => void
}): React.JSX.Element {
  const pending = useRef<Addition | undefined>(undefined)

  async function add(fields: DateFields): Promise<void> {
    const previous = pending.current ?? readPending()

    pending.current =
      previous !== undefined && sameFields(previous, fields)
        ? previous
        : { action: "add", ...fields, request_id: createRequestId() }

    storePending(pending.current)

    if (await save(pending.current, `${fields.title} added.`)) {
      pending.current = undefined
      storePending(undefined)
      done()
    }
  }

  return (
    <DateForm
      owners={owners}
      initial={{
        title: "",
        date: "",
        kind: "since",
        person_ids: person === undefined ? [] : [person],
        show_when_idle: false,
      }}
      busy={busy}
      label="Add date"
      submit={(fields) => void add(fields)}
    />
  )
}

/**
 * One date: its live figure large, then when it was and who it is about. The idle switch saves at
 * once; Edit opens the same fields as adding, and Delete asks first.
 */
export function DateDetail({
  date,
  owners,
  busy,
  save,
  done,
}: {
  date: ImportantDate
  owners: readonly HouseholdUser[]
  busy: boolean
  save: Save
  done: () => void
}): React.JSX.Element {
  const [mode, setMode] = useState<"view" | "edit" | "delete">("view")
  const target = { action: "edit" as const, date_id: date.id }

  const tagged = date.person_ids
    .map((id) => owners.find((owner) => owner.id === id))
    .filter((owner) => owner !== undefined)

  if (mode === "edit")
    return (
      <DateForm
        owners={owners}
        initial={{
          title: date.title,
          date: date.date,
          kind: date.kind,
          person_ids: date.person_ids,
          show_when_idle: date.show_when_idle,
        }}
        busy={busy}
        label="Save date"
        cancel={() => setMode("view")}
        submit={(fields) => {
          const same = (ids: readonly string[]) =>
            ids.length === date.person_ids.length &&
            ids.every((id) => date.person_ids.includes(id))

          void save(
            {
              ...target,
              title: fields.title === date.title ? undefined : fields.title,
              date: fields.date === date.date ? undefined : fields.date,
              kind: fields.kind === date.kind ? undefined : fields.kind,
              person_ids: same(fields.person_ids)
                ? undefined
                : fields.person_ids,
              show_when_idle:
                fields.show_when_idle === date.show_when_idle
                  ? undefined
                  : fields.show_when_idle,
            },
            `${fields.title} saved.`
          ).then((saved) => {
            if (saved) setMode("view")
          })
        }}
      />
    )

  return (
    <div className="sheet-body">
      <p className="sheet-owner">
        <DateKindGlyph kind={date.kind} />
        {tagged.length === 0 ? (
          <span>Everyone</span>
        ) : (
          tagged.map((owner) => (
            <span key={owner.id} className="date-person">
              <PersonDisc name={owner.name} />
              {owner.name}
            </span>
          ))
        )}
      </p>
      {/* The sheet's title already names the date for assistive technology. */}
      <p className="sheet-name" aria-hidden="true">
        {date.title}
      </p>
      <p>
        <DateFigure date={date} large />
      </p>
      <dl className="watch-days date-days">
        <div>
          <dt>{date.kind === "since" ? "Since" : "Date"}</dt>
          <dd className="num">
            <time dateTime={date.date}>{formatDay(date.date)}</time>
          </dd>
        </div>
        {date.kind === "since" && date.elapsed !== null ? (
          <div>
            <dt>Days</dt>
            <dd className="num">
              {date.elapsed.total_days.toLocaleString("en-GB")}
            </dd>
          </div>
        ) : null}
        <div>
          <dt>{date.kind === "since" ? "Next anniversary" : "Next"}</dt>
          <dd className="num">
            <time dateTime={date.next.date}>{formatDay(date.next.date)}</time>
            {date.next.years === 0 ? null : (
              <span className="date-years">
                {" "}
                · {date.next.years} {date.next.years === 1 ? "year" : "years"}
              </span>
            )}
          </dd>
        </div>
      </dl>
      <IdleSwitch
        checked={date.show_when_idle}
        disabled={busy}
        toggle={(show_when_idle) =>
          void save(
            { ...target, show_when_idle },
            show_when_idle
              ? `${date.title} shows when idle.`
              : `${date.title} is hidden when idle.`
          )
        }
      />
      {mode === "delete" ? (
        <div className="sheet-confirm" role="group" aria-label="Delete date">
          <p>Delete “{date.title}” for everyone? This cannot be undone.</p>
          <div className="sheet-actions">
            <Button
              variant="destructive"
              size="key"
              disabled={busy}
              onClick={() => {
                void save(
                  { action: "remove", date_id: date.id },
                  `${date.title} deleted.`
                ).then((saved) => {
                  if (saved) done()
                })
              }}
            >
              <TrashIcon size={18} aria-hidden="true" />
              Delete for good
            </Button>
            <Button
              variant="ghost"
              size="key"
              disabled={busy}
              onClick={() => setMode("view")}
            >
              Keep it
            </Button>
          </div>
        </div>
      ) : (
        <div className="sheet-actions sheet-actions-end">
          <Button
            variant="outline"
            size="key"
            disabled={busy}
            onClick={() => setMode("edit")}
          >
            <PencilSimpleIcon size={18} aria-hidden="true" />
            Edit
          </Button>
          <Button
            variant="destructive"
            size="key"
            disabled={busy}
            onClick={() => setMode("delete")}
          >
            <TrashIcon size={18} aria-hidden="true" />
            Delete
          </Button>
        </div>
      )}
    </div>
  )
}
