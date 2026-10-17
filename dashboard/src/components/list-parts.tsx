import { useRef, useState } from "react"
import { Dialog } from "@base-ui/react/dialog"
import { UsersThreeIcon } from "@phosphor-icons/react/dist/csr/UsersThree"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import { initial, type ListFeedback } from "@/lib/lists"
import type { HouseholdUser } from "@/lib/wishlist"

/**
 * Everyone, then one key per Household User, on one wash track. The chosen key is the ink
 * pill; counts are figures with their words kept for assistive technology.
 */
export function PersonKeys({
  owners,
  selected,
  choose,
  counts,
  noun,
  label = "Whose list",
}: {
  owners: readonly HouseholdUser[]
  selected: string | undefined
  choose: (id: string | undefined) => void
  counts?: ReadonlyMap<string, number>
  noun?: readonly [string, string]
  label?: string
}): React.JSX.Element {
  return (
    <div className="view-tabs person-keys" role="group" aria-label={label}>
      <button
        type="button"
        className="view-tab person-key"
        aria-pressed={selected === undefined}
        onClick={() => choose(undefined)}
      >
        <span className="person-disc" aria-hidden="true">
          <UsersThreeIcon size={16} weight="bold" />
        </span>
        Everyone
      </button>
      {owners.map((owner) => {
        const count = counts?.get(owner.id)

        return (
          <button
            key={owner.id}
            type="button"
            className="view-tab person-key"
            aria-pressed={selected === owner.id}
            onClick={() => choose(owner.id)}
          >
            <PersonDisc name={owner.name} />
            {owner.name}
            {count === undefined || noun === undefined ? null : (
              <span className="person-count num">
                {count}
                <span className="sr-only">
                  {" "}
                  {count === 1 ? noun[0] : noun[1]}
                </span>
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}

/** A person's initial in a disc; decorative, since the name is always said beside it. */
export function PersonDisc({ name }: { name: string }): React.JSX.Element {
  return (
    <span className="person-disc" aria-hidden="true">
      {initial(name)}
    </span>
  )
}

/**
 * A detail plate that rises from the right edge (from the bottom on a phone): one tone step
 * above the page, over a ground-coloured scrim. No border, shadow or blur.
 */
export function ListSheet({
  open,
  close,
  title,
  titleHidden = false,
  children,
}: {
  open: boolean
  close: () => void
  title: string
  titleHidden?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) close()
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop className="sheet-scrim" />
        <Dialog.Popup className="sheet">
          <div className="sheet-head">
            <Dialog.Title className={titleHidden ? "sr-only" : "sheet-title"}>
              {title}
            </Dialog.Title>
            <Dialog.Close render={<Button variant="outline" size="key-icon" />}>
              <XIcon size={20} aria-hidden="true" />
              <span className="sr-only">Close</span>
            </Dialog.Close>
          </div>
          {children}
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

/** A segmented choice on a wash track; the chosen segment is the ink pill. */
export function Segments<T extends string>({
  label,
  value,
  options,
  choose,
  disabled = false,
}: {
  label: string
  value: T | undefined
  options: ReadonlyArray<{
    value: T
    label: string
    icon?: React.ReactNode
  }>
  choose: (value: T) => void
  disabled?: boolean
}): React.JSX.Element {
  return (
    <div className="view-tabs segments" role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className="view-tab"
          aria-pressed={value === option.value}
          disabled={disabled}
          onClick={() => choose(option.value)}
        >
          {option.icon}
          {option.label}
        </button>
      ))}
    </div>
  )
}

/** Visible only when something failed; success is said to assistive technology alone. */
export function FeedbackLine({
  feedback,
  busy,
}: {
  feedback: ListFeedback | undefined
  busy: boolean
}): React.JSX.Element {
  return (
    <p
      className={feedback?.failed === true ? "list-failure" : "sr-only"}
      role="status"
    >
      {feedback?.text ?? (busy ? "Saving…" : "")}
    </p>
  )
}

type Receipt<Reason extends string> =
  | { readonly state: "saved" }
  | { readonly state: "failed"; readonly reason: Reason }

/** The save in progress, what it last said, and the save itself. */
export type ListSave<Command> = {
  busy: boolean
  feedback: ListFeedback | undefined
  setFeedback: (feedback: ListFeedback | undefined) => void
  save: (command: Command, saved: string) => Promise<boolean>
}

/**
 * One write at a time: send, then re-read the view. A lost response is reported as uncertain,
 * never as saved; a saved change that could not re-read says so.
 */
export function useListSave<Command, Reason extends string>(
  change: (command: Command) => Promise<Receipt<Reason>>,
  refresh: () => Promise<void>,
  messages: Record<Reason, string>,
  unreachable: Reason
): ListSave<Command> {
  const [busy, setBusy] = useState(false)
  const [feedback, setFeedback] = useState<ListFeedback>()
  const locked = useRef(false)

  async function save(command: Command, saved: string): Promise<boolean> {
    if (locked.current) return false
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      let result: Receipt<Reason>

      try {
        result = await change(command)
      } catch {
        result = { state: "failed", reason: unreachable }
      }

      if (result.state === "failed") {
        setFeedback({ failed: true, text: messages[result.reason] })

        return false
      }

      try {
        await refresh()
        setFeedback({ failed: false, text: saved })
      } catch {
        setFeedback({
          failed: true,
          text: "Saved to OpenHome, but the view could not update. Reopen the page to see it.",
        })
      }

      return true
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return { busy, feedback, setFeedback, save }
}
