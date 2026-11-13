import { useEffect, useId, useRef, useState } from "react"
import { CircleNotchIcon } from "@phosphor-icons/react/dist/csr/CircleNotch"
import { MagnifyingGlassIcon } from "@phosphor-icons/react/dist/csr/MagnifyingGlass"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import {
  FeedbackLine,
  ListSheet,
  PersonDisc,
  Segments,
  useListSave,
  useSheetLifetime,
} from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cardImageUrl, type BinderEntry } from "@/lib/binder"
import {
  additionOf,
  advance,
  binderAddFailureMessage,
  persist,
  recover,
  type AddFlow,
  type AddStep,
  type BinderAddReceipt,
  type BinderAddition,
  type CardSearchResult,
  type CardVariant,
  type Draft,
} from "@/lib/binder-add"
import { trimText } from "@/lib/lists"
import { createRequestId } from "@/lib/request-id"
import type { HouseholdUser } from "@/lib/wishlist"

export type FindCards = (input: {
  card_code: string
}) => Promise<CardSearchResult>

/** The flow, stepped through the one transition function and kept in storage before it renders. */
function useAddFlow(): [AddFlow, (step: AddStep) => void] {
  const [flow, setFlow] = useState<AddFlow>(recover)
  const latest = useRef(flow)

  function step(next: AddStep): void {
    const to = advance(latest.current, next)
    latest.current = to
    persist(to)
    setFlow(to)
  }

  return [flow, step]
}

const searchMessage: Record<
  "searching" | "unlisted" | "invalid" | "outage" | "failed",
  (code: string) => string
> = {
  searching: () => "Searching the catalogue…",
  unlisted: (code) => `${code} is not in the catalogue yet.`,
  invalid: () => "That is not a Card Code. Try one like OP01-001.",
  outage: () =>
    "The card catalogue is not answering. Try again in a moment; you can search again.",
  failed: () => "OpenHome could not search the catalogue. Try again.",
}

const languages = [
  { value: "en", label: "EN" },
  { value: "ja", label: "JP" },
] as const

/** The Binder's "Add card": a button that opens the sheet, and the write behind it. */
export function AddCard({
  owners,
  person,
  entries,
  find,
  change,
  refresh,
}: {
  owners: readonly HouseholdUser[]
  person: string
  entries: readonly BinderEntry[]
  find: FindCards
  change: (command: BinderAddition) => Promise<BinderAddReceipt>
  refresh: () => Promise<void>
}): React.JSX.Element {
  const { busy, feedback, save } = useListSave(
    change,
    refresh,
    binderAddFailureMessage,
    "unreachable"
  )

  const [open, setOpen] = useState(false)

  // A refresh resumes an unconfirmed addition rather than silently dropping it.
  useEffect(() => {
    // eslint-disable-next-line react/set-state-in-effect -- Session storage exists only in the browser; reading it in render would not match the server's render.
    if (recover().state !== "idle") setOpen(true)
  }, [])

  return (
    <>
      <Button size="key" onClick={() => setOpen(true)}>
        <PlusIcon size={20} weight="bold" aria-hidden="true" />
        Add card
      </Button>
      <ListSheet open={open} close={() => setOpen(false)} title="Add a card">
        {open ? (
          <>
            <AddSheet
              owners={owners}
              person={person}
              entries={entries}
              busy={busy}
              find={find}
              save={save}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
    </>
  )
}

function AddSheet({
  owners,
  person,
  entries,
  busy,
  find,
  save,
}: {
  owners: readonly HouseholdUser[]
  person: string
  entries: readonly BinderEntry[]
  busy: boolean
  find: FindCards
  save: (command: BinderAddition, saved: string) => Promise<boolean>
}): React.JSX.Element {
  const id = useId()
  const [flow, step] = useAddFlow()
  const [code, setCode] = useState("")
  const alive = useSheetLifetime()

  async function search(text: string): Promise<void> {
    const wanted = trimText(text)

    if (wanted === "") return
    step({ type: "search", code: wanted })

    const result = await find({ card_code: wanted }).catch(
      (): CardSearchResult => ({ state: "failed" })
    )

    if (alive()) step({ type: "searched", code: wanted, result })
  }

  async function add(draft: Draft): Promise<void> {
    step({ type: "sent" })

    if (await save(additionOf(draft), `Added ${draft.card.name}.`))
      step({ type: "saved" })
  }

  if (flow.state === "drafting" || flow.state === "unconfirmed")
    return (
      <Drafting
        draft={flow.draft}
        locked={flow.state === "unconfirmed"}
        owners={owners}
        busy={busy}
        edit={(change) => step({ type: "edit", change })}
        add={() => void add(flow.draft)}
        discard={() => step({ type: "discard" })}
      />
    )

  if (flow.state === "added")
    return (
      <Added
        draft={flow.draft}
        owner={owners.find((owner) => owner.id === flow.draft.owner_id)}
        stored={entries.find(
          (entry) =>
            entry.owner_id === flow.draft.owner_id &&
            entry.card.id === flow.draft.card.id &&
            entry.language === flow.draft.language
        )}
        another={() => {
          step({ type: "discard" })
          setCode("")
        }}
      />
    )

  return (
    <form
      className="sheet-body"
      aria-label="Search by Card Code"
      onSubmit={(event) => {
        event.preventDefault()
        void search(code)
      }}
    >
      <label className="field" htmlFor={`${id}-code`}>
        <span>Card Code</span>
        <span className="link-input">
          <MagnifyingGlassIcon size={22} aria-hidden="true" />
          <Input
            id={`${id}-code`}
            name="card_code"
            value={code}
            autoFocus
            autoComplete="off"
            autoCapitalize="characters"
            maxLength={32}
            placeholder="OP01-001"
            onChange={(event) => setCode(event.target.value)}
          />
          {flow.state === "searching" ? (
            <CircleNotchIcon size={22} className="spin" aria-hidden="true" />
          ) : null}
        </span>
      </label>
      <div className="sheet-actions">
        <Button
          type="submit"
          disabled={flow.state === "searching" || trimText(code) === ""}
        >
          Search
        </Button>
      </div>
      <p
        className={
          flow.state === "idle" || flow.state === "found"
            ? "sr-only"
            : "binder-search-note"
        }
        role="status"
      >
        {flow.state === "idle"
          ? ""
          : flow.state === "found"
            ? `${flow.cards.length} ${flow.cards.length === 1 ? "variant" : "variants"} of ${flow.code}`
            : searchMessage[flow.state](flow.code)}
      </p>
      {flow.state === "found" ? (
        <ul className="card-results" aria-label={`Variants of ${flow.code}`}>
          {flow.cards.map((card) => (
            <li key={card.id}>
              <button
                type="button"
                className="card-result"
                onClick={() =>
                  step({
                    type: "pick",
                    card,
                    owner_id: person,
                    request_id: createRequestId(),
                  })
                }
              >
                <CardArt card={card} />
                <span className="media-result-title">{card.name}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </form>
  )
}

/** The official image over the card's code, which stays if the picture cannot load. */
function CardArt({ card }: { card: CardVariant }): React.JSX.Element {
  return (
    <span className="card-art">
      <span className="card-art-code num" aria-hidden="true">
        {card.card_code}
      </span>
      <img
        src={cardImageUrl(card.id)}
        alt=""
        width={600}
        height={838}
        onError={(event) => {
          event.currentTarget.hidden = true
        }}
        onLoad={(event) => {
          event.currentTarget.hidden = false
        }}
      />
    </span>
  )
}

function Drafting({
  draft,
  locked,
  owners,
  busy,
  edit,
  add,
  discard,
}: {
  draft: Draft
  locked: boolean
  owners: readonly HouseholdUser[]
  busy: boolean
  edit: (
    change: Partial<Pick<Draft, "owner_id" | "quantity" | "language">>
  ) => void
  add: () => void
  discard: () => void
}): React.JSX.Element {
  const id = useId()
  const frozen = busy || locked

  return (
    <form
      className="sheet-body"
      aria-label="Add this card"
      onSubmit={(event) => {
        event.preventDefault()

        if (!busy) add()
      }}
    >
      <div className="binder-draft">
        <CardArt card={draft.card} />
        <div>
          <p className="sheet-name">{draft.card.name}</p>
          <p className="media-result-meta num">{draft.card.card_code}</p>
          {locked ? (
            <p className="binder-search-note" role="status">
              This addition is not confirmed yet. Retrying it will not add the
              card twice.
            </p>
          ) : null}
        </div>
      </div>
      <fieldset className="field destination">
        <legend>Add to</legend>
        <div className="view-tabs person-keys">
          {owners.map((owner) => (
            <button
              key={owner.id}
              type="button"
              className="view-tab person-key"
              aria-pressed={draft.owner_id === owner.id}
              disabled={frozen}
              onClick={() => edit({ owner_id: owner.id })}
            >
              <PersonDisc name={owner.name} />
              {owner.name}
            </button>
          ))}
        </div>
      </fieldset>
      <div className="field">
        <span id={`${id}-quantity`}>Quantity</span>
        <div
          className="stepper"
          role="group"
          aria-labelledby={`${id}-quantity`}
        >
          <Button
            type="button"
            variant="outline"
            size="key-icon"
            disabled={frozen || draft.quantity <= 1}
            onClick={() => edit({ quantity: draft.quantity - 1 })}
          >
            <MinusIcon size={18} aria-hidden="true" />
            <span className="sr-only">One fewer</span>
          </Button>
          <output className="num" aria-live="polite">
            {draft.quantity}
          </output>
          <Button
            type="button"
            variant="outline"
            size="key-icon"
            disabled={frozen}
            onClick={() => edit({ quantity: draft.quantity + 1 })}
          >
            <PlusIcon size={18} aria-hidden="true" />
            <span className="sr-only">One more</span>
          </Button>
        </div>
      </div>
      <div className="field">
        <span>Language</span>
        <Segments
          label="Language"
          value={draft.language}
          options={languages}
          choose={(language) => edit({ language })}
          disabled={frozen}
        />
      </div>
      <div className="sheet-actions sheet-submit">
        <Button type="submit" size="key-xl" disabled={busy}>
          <PlusIcon size={20} weight="bold" aria-hidden="true" />
          {locked ? "Retry add" : "Add card"}
        </Button>
        <Button type="button" variant="ghost" disabled={busy} onClick={discard}>
          Discard draft
        </Button>
      </div>
    </form>
  )
}

/** What the Binder now holds, read back from the refreshed Binder rather than assumed. */
function Added({
  draft,
  owner,
  stored,
  another,
}: {
  draft: Draft
  owner: HouseholdUser | undefined
  stored: BinderEntry | undefined
  another: () => void
}): React.JSX.Element {
  const whose = owner === undefined ? "the" : `${owner.name}’s`

  return (
    <div className="sheet-body">
      <div className="binder-draft">
        <CardArt card={draft.card} />
        <div>
          <p className="sheet-name">{draft.card.name}</p>
          <p className="media-result-meta num">{draft.card.card_code}</p>
          <p role="status" className="binder-added">
            {stored === undefined
              ? `Added to ${whose} Binder.`
              : stored.quantity > draft.quantity
                ? `Quantity raised to ${stored.quantity} in ${whose} Binder.`
                : `Added to ${whose} Binder, quantity ${stored.quantity}.`}
          </p>
        </div>
      </div>
      <div className="sheet-actions sheet-submit">
        <Button type="button" size="key" onClick={another}>
          Add another card
        </Button>
      </div>
    </div>
  )
}
