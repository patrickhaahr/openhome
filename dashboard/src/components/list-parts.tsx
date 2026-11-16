import { useEffect, useReducer, useRef } from "react"
import { Dialog } from "@base-ui/react/dialog"
import { cva, type VariantProps } from "class-variance-authority"
import { UsersThreeIcon } from "@phosphor-icons/react/dist/csr/UsersThree"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { useIdleBlock } from "@/components/idle-block"
import { sheetScrim } from "@/components/sheet-parts"
import { Button } from "@/components/ui/button"
import { ViewTabs, viewTab } from "@/components/view-tabs"
import { initial, type ListFeedback } from "@/lib/lists"
import type { HouseholdUser } from "@/lib/wishlist"

/** A person's disc; `ground` is for a disc on a tone step, and a wash disc turns ground inside a chosen key. */
const disc = cva(
  "inline-grid flex-none place-items-center rounded-full leading-none font-semibold text-ink",
  {
    variants: {
      size: {
        default: "size-8 text-sm",
        heading: "size-9 text-base tracking-normal",
        plate: "size-7 text-label",
        household: "size-11 text-sm",
        key: "size-14 text-2xl",
      },
      tone: {
        wash: "bg-wash-strong group-aria-pressed/tab:bg-ground",
        ground: "bg-ground",
      },
    },
    defaultVariants: { size: "default", tone: "wash" },
  }
)

const sheet = cva(
  "fixed z-51 grid overscroll-contain text-ink duration-fast ease-app outline-none max-sm:max-h-sheet-height max-sm:rounded-t-sheet",
  {
    variants: {
      variant: {
        plate:
          "inset-y-0 right-0 w-sheet content-start gap-5 overflow-y-auto bg-plate px-gutter pt-4 pb-8 transition-transform data-ending-style:translate-x-full data-starting-style:translate-x-full max-sm:top-auto max-sm:left-0 max-sm:w-full max-sm:data-ending-style:translate-x-0 max-sm:data-ending-style:translate-y-full max-sm:data-starting-style:translate-x-0 max-sm:data-starting-style:translate-y-full",
        /** The Binder's card stage: the whole screen, fading in rather than rising. */
        stage:
          "inset-0 grid-rows-binder-stage content-stretch overflow-hidden bg-binder-stage transition-binder-stage data-ending-style:scale-98 data-ending-style:opacity-0 data-starting-style:scale-98 data-starting-style:opacity-0",
      },
    },
    defaultVariants: { variant: "plate" },
  }
)

const sheetHead = cva("flex min-h-12 items-center justify-end gap-4", {
  variants: { variant: { plate: "", stage: "px-binder-stage-inset py-4" } },
  defaultVariants: { variant: "plate" },
})

const sheetTitle = cva("mr-auto font-medium", {
  variants: {
    variant: {
      plate: "text-lead tracking-title",
      stage: "min-w-0 text-binder-card-title tracking-headline",
    },
  },
  defaultVariants: { variant: "plate" },
})

/** `rail` stands the keys in the Binder's left gutter on a wide screen. */
const personKeys = cva("", {
  variants: {
    variant: {
      row: "",
      rail: "binder-wide:col-start-1 binder-wide:row-span-full binder-wide:flex-col binder-wide:self-start binder-wide:justify-self-start binder-wide:rounded-panel",
    },
  },
  defaultVariants: { variant: "row" },
})

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
  everyone = true,
  variant,
}: {
  owners: readonly HouseholdUser[]
  selected: string | undefined
  choose: (id: string | undefined) => void
  counts?: ReadonlyMap<string, number>
  noun?: readonly [string, string]
  label?: string
  everyone?: boolean
  variant?: VariantProps<typeof personKeys>["variant"]
}): React.JSX.Element {
  return (
    <ViewTabs
      variant="people"
      className={personKeys({ variant })}
      data-slot="person-keys"
      role="group"
      aria-label={label}
    >
      {everyone ? (
        <button
          type="button"
          {...viewTab("person")}
          aria-pressed={selected === undefined}
          onClick={() => choose(undefined)}
        >
          <span className={disc()} aria-hidden="true">
            <UsersThreeIcon size={16} weight="bold" />
          </span>
          Everyone
        </button>
      ) : null}
      {owners.map((owner) => {
        const count = counts?.get(owner.id)

        return (
          <button
            key={owner.id}
            type="button"
            {...viewTab("person")}
            aria-pressed={selected === owner.id}
            onClick={() => choose(owner.id)}
          >
            <PersonDisc name={owner.name} />
            {owner.name}
            {count === undefined || noun === undefined ? null : (
              <span className="text-label num text-muted-ink group-aria-pressed/tab:text-ground/70">
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
    </ViewTabs>
  )
}

/** A person's initial in a disc; decorative, since the name is always said beside it. */
export function PersonDisc({
  name,
  size,
  tone,
}: { name: string } & VariantProps<typeof disc>): React.JSX.Element {
  return (
    <span className={disc({ size, tone })} aria-hidden="true">
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
  variant,
  actions,
  onKeyDown,
  children,
}: {
  open: boolean
  close: () => void
  title: string
  titleHidden?: boolean
  variant?: VariantProps<typeof sheet>["variant"]
  actions?: React.ReactNode
  onKeyDown?: React.KeyboardEventHandler<HTMLDivElement>
  children: React.ReactNode
}): React.JSX.Element {
  // An open sheet holds a detail, an edit or an add flow: never hide it behind the slideshow.
  useIdleBlock(open)

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) close()
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop className={sheetScrim} />
        <Dialog.Popup
          data-slot="sheet"
          className={sheet({ variant })}
          onKeyDown={onKeyDown}
        >
          <div data-slot="sheet-head" className={sheetHead({ variant })}>
            <Dialog.Title
              data-slot="sheet-title"
              className={titleHidden ? "sr-only" : sheetTitle({ variant })}
            >
              {title}
            </Dialog.Title>
            {actions}
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
  fit = false,
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
  fit?: boolean
}): React.JSX.Element {
  return (
    <ViewTabs
      variant={fit ? "fit" : "segments"}
      role="group"
      aria-label={label}
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          {...viewTab("segment")}
          aria-pressed={value === option.value}
          disabled={disabled}
          onClick={() => choose(option.value)}
        >
          {option.icon}
          {option.label}
        </button>
      ))}
    </ViewTabs>
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
  const failed = feedback?.failed === true

  return (
    <p
      data-slot={failed ? "list-failure" : undefined}
      className={
        failed
          ? "flex items-baseline gap-2 text-control text-danger before:size-2 before:flex-none before:-translate-y-0.5 before:rounded-full before:bg-current"
          : "sr-only"
      }
      role="status"
    >
      {feedback?.text ?? (busy ? "Saving…" : "")}
    </p>
  )
}

/**
 * Whether the calling sheet content is still mounted. A write outlives the sheet that sent it,
 * and the board's `done` closes whichever sheet is open, so only a completion that finds its
 * own sheet still mounted may call `done`. Durable work, such as clearing a spent retry
 * intention, runs either way, because the write really was saved.
 */
export function useSheetLifetime(): () => boolean {
  const mounted = useRef(false)

  useEffect(() => {
    mounted.current = true

    return () => {
      mounted.current = false
    }
  }, [])

  return () => mounted.current
}

type Receipt<Reason extends string> =
  | { readonly state: "saved" }
  | { readonly state: "failed"; readonly reason: Reason }

/** What a finished save said; each outcome carries exactly the data its feedback needs. */
type Outcome<Reason extends string> =
  | { readonly result: "rejected"; readonly reason: Reason }
  | { readonly result: "saved"; readonly text: string }
  | { readonly result: "saved-unread" }

/**
 * One save, from request to outcome. `sending` and `refreshing` keep the acknowledgement text
 * until the outcome needs it; `settled` is the only phase that carries feedback.
 */
type Operation<Reason extends string> =
  | { readonly phase: "idle" }
  | { readonly phase: "sending"; readonly saved: string }
  | { readonly phase: "refreshing"; readonly saved: string }
  | { readonly phase: "settled"; readonly outcome: Outcome<Reason> }

type Step<Reason extends string> =
  | { readonly type: "send"; readonly saved: string }
  | { readonly type: "rejected"; readonly reason: Reason }
  | { readonly type: "acknowledged" }
  | { readonly type: "refreshed" }
  | { readonly type: "unread" }

/**
 * The only place an Operation is constructed. A step that does not follow the current phase
 * leaves it alone, so a late or repeated step can neither revive nor overwrite a settled save.
 */
function advance<Reason extends string>(
  operation: Operation<Reason>,
  step: Step<Reason>
): Operation<Reason> {
  switch (step.type) {
    case "send":
      return operation.phase === "sending" || operation.phase === "refreshing"
        ? operation
        : { phase: "sending", saved: step.saved }
    case "rejected":
      return operation.phase === "sending"
        ? {
            phase: "settled",
            outcome: { result: "rejected", reason: step.reason },
          }
        : operation
    case "acknowledged":
      return operation.phase === "sending"
        ? { phase: "refreshing", saved: operation.saved }
        : operation
    case "refreshed":
      return operation.phase === "refreshing"
        ? {
            phase: "settled",
            outcome: { result: "saved", text: operation.saved },
          }
        : operation
    case "unread":
      return operation.phase === "refreshing"
        ? { phase: "settled", outcome: { result: "saved-unread" } }
        : operation
  }
}

function feedbackOf<Reason extends string>(
  operation: Operation<Reason>,
  messages: Record<Reason, string>
): ListFeedback | undefined {
  if (operation.phase !== "settled") return undefined

  switch (operation.outcome.result) {
    case "rejected":
      return { failed: true, text: messages[operation.outcome.reason] }
    case "saved":
      return { failed: false, text: operation.outcome.text }
    case "saved-unread":
      return {
        failed: true,
        text: "Saved to OpenHome, but the view could not update. Reopen the page to see it.",
      }
  }
}

/** The save in progress, what it last said, and the save itself. */
export type ListSave<Command> = {
  busy: boolean
  feedback: ListFeedback | undefined
  save: (command: Command, saved: string) => Promise<boolean>
}

/**
 * One write at a time: send, then re-read the view. A lost response is reported as uncertain,
 * never as saved; a saved change that could not re-read says so, and still resolves `true`.
 *
 * The board that calls this hook outlives its sheets, so a save may settle after the sheet that
 * sent it has closed; that sheet's completion is guarded by `useSheetLifetime`, not here.
 *
 * `save` resolves once the operation has settled. `change` and `refresh` each settle by
 * resolving or rejecting, which always releases the lock. Their time limits belong to the
 * transport: a call that never settles keeps the form busy until the page is reloaded, and no
 * timeout is added here because none is required by a source.
 */
export function useListSave<Command, Reason extends string>(
  change: (command: Command) => Promise<Receipt<Reason>>,
  refresh: () => Promise<void>,
  messages: Record<Reason, string>,
  unreachable: Reason
): ListSave<Command> {
  const [operation, step] = useReducer<Operation<Reason>, [Step<Reason>]>(
    advance,
    { phase: "idle" }
  )

  // The reducer's state lags a render; this closes the gap for a second call in the same tick.
  const locked = useRef(false)

  async function save(command: Command, saved: string): Promise<boolean> {
    if (locked.current) return false
    locked.current = true
    step({ type: "send", saved })

    try {
      let result: Receipt<Reason>

      try {
        result = await change(command)
      } catch {
        result = { state: "failed", reason: unreachable }
      }

      if (result.state === "failed") {
        step({ type: "rejected", reason: result.reason })

        return false
      }

      step({ type: "acknowledged" })

      try {
        await refresh()
        step({ type: "refreshed" })
      } catch {
        step({ type: "unread" })
      }

      return true
    } finally {
      locked.current = false
    }
  }

  return {
    busy: operation.phase === "sending" || operation.phase === "refreshing",
    feedback: feedbackOf(operation, messages),
    save,
  }
}
