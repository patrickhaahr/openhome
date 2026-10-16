import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { useId, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import {
  watchlistEntry,
  watchStatusLabels,
  type WatchlistCommand,
  type WatchlistEntry,
} from "@/lib/watchlist"

type WatchlistEntryDetailsProps = {
  entry: WatchlistEntry
  ownerName: string
  disabled: boolean
  failure: string | undefined
  save: (command: WatchlistCommand) => Promise<boolean>
}

/** Owner-labelled date corrections; the API owns defaults and transitions. */
export function WatchlistEntryDetails({
  entry,
  ownerName,
  disabled,
  failure,
  save,
}: WatchlistEntryDetailsProps): React.JSX.Element {
  const id = useId()
  const details = useRef<HTMLDetailsElement>(null)
  const [status, setStatus] = useState(entry.status)
  const [dateAdded, setDateAdded] = useState(entry.date_added)

  const [completionDate, setCompletionDate] = useState(
    entry.completion_date ?? ""
  )

  const [failed, setFailed] = useState(false)

  function reset(): void {
    setStatus(entry.status)
    setDateAdded(entry.date_added)
    setCompletionDate(entry.completion_date ?? "")
    setFailed(false)
  }

  async function submit(): Promise<void> {
    const saved = await save({
      action: "edit",
      owner_id: entry.owner_id,
      entry_id: entry.id,
      status: status === entry.status ? undefined : status,
      date_added: dateAdded === entry.date_added ? undefined : dateAdded,
      completion_date:
        status === "completed" &&
        completionDate !== "" &&
        completionDate !== entry.completion_date
          ? completionDate
          : undefined,
    })

    setFailed(!saved)

    if (saved && details.current) details.current.open = false
  }

  return (
    <details
      ref={details}
      className="watchlist-edit"
      onToggle={(event) => {
        if (event.currentTarget.open) reset()
      }}
    >
      <summary
        aria-label={`Edit status and dates for ${entry.title} in ${ownerName}’s watchlist`}
      >
        <PencilSimpleIcon size={18} aria-hidden="true" />
        <span className="sr-only">Edit status &amp; dates</span>
      </summary>
      <form
        aria-label={`Edit ${entry.title} for ${ownerName}`}
        onSubmit={(event) => {
          event.preventDefault()

          if (!disabled) void submit()
        }}
      >
        <div>
          <label htmlFor={`${id}-status`}>
            Status for {entry.title} / {ownerName}
          </label>
          <NativeSelect
            id={`${id}-status`}
            value={status}
            disabled={disabled}
            onChange={(event) => {
              const value = watchlistEntry.shape.status.parse(
                event.target.value
              )

              setStatus(value)

              if (value !== "completed") setCompletionDate("")
            }}
          >
            {Object.entries(watchStatusLabels).map(([value, label]) => (
              <NativeSelectOption key={value} value={value}>
                {label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
        <div>
          <label htmlFor={`${id}-added`}>
            Date added for {entry.title} / {ownerName}
          </label>
          <Input
            id={`${id}-added`}
            type="date"
            value={dateAdded}
            required
            disabled={disabled}
            onChange={(event) => setDateAdded(event.target.value)}
          />
        </div>
        {status === "completed" ? (
          <div>
            <label htmlFor={`${id}-completed`}>
              Completion date for {entry.title} / {ownerName}
            </label>
            <Input
              id={`${id}-completed`}
              type="date"
              min={dateAdded}
              value={completionDate}
              disabled={disabled}
              aria-describedby={`${id}-date-hint`}
              onChange={(event) => setCompletionDate(event.target.value)}
            />
            <p id={`${id}-date-hint`}>
              Leave blank to keep the saved date, or use today when first
              completed.
            </p>
          </div>
        ) : (
          <p>Moving away from completed clears its completion date.</p>
        )}
        {failed ? (
          <p role="status" className="wishlist-feedback" data-failed="true">
            {failure ??
              "Could not confirm the change. Refresh before retrying."}
          </p>
        ) : null}
        <div className="watchlist-edit-actions">
          <Button type="submit" disabled={disabled}>
            Save status &amp; dates
          </Button>
          <Button
            type="button"
            variant="ghost"
            disabled={disabled}
            onClick={() => {
              reset()

              if (details.current) details.current.open = false
            }}
          >
            Cancel edit
          </Button>
        </div>
      </form>
    </details>
  )
}
