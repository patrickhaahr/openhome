import { useEffect, useState } from "react"
import { Link } from "@tanstack/react-router"
import { CardsIcon } from "@phosphor-icons/react/dist/csr/Cards"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { CheckAgainButton } from "@/components/check-again-button"
import { OpenConnection } from "@/components/control-alert"
import { BinderSheet } from "@/components/binder-sheet"
import { BinderValueBar } from "@/components/binder-value"
import {
  FeedbackLine,
  ListSheet,
  PersonKeys,
  useListSave,
} from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import {
  binderFailureMessage,
  cardImageUrl,
  pocketsPerPage,
  viewOf,
  type BinderEntry,
  type BinderSearch,
  type BinderState,
  type BinderView,
} from "@/lib/binder"
import {
  binderWriteMessage,
  type BinderCommand,
  type BinderMutation,
} from "@/lib/binder-edit"

/**
 * One Household User's Binder, a spread of two facing 3×3 pages at a time in Card Code order.
 * The person keys choose whose Binder; the arrows and the arrow keys turn the spread, and a
 * pocket opens its card sheet. A read that failed offers a way to check again and never shows
 * an empty Binder.
 */
export function BinderBook({
  data,
  search,
  choose,
  change,
  refresh,
}: {
  data: BinderState
  search: BinderSearch
  choose: (search: BinderSearch) => void
  change: (command: BinderCommand) => Promise<BinderMutation>
  refresh: () => Promise<void>
}): React.JSX.Element {
  const view = viewOf(data, search)
  const [open, setOpen] = useState<string>()

  const { busy, feedback, save } = useListSave(
    change,
    refresh,
    binderWriteMessage,
    "unreachable"
  )

  const selected =
    data.state === "available" && open !== undefined
      ? data.entries.find((entry) => entry.id === open)
      : undefined

  const owner =
    data.state === "available" && selected !== undefined
      ? data.owners.find((candidate) => candidate.id === selected.owner_id)
      : undefined

  return (
    <div className="lists binder" aria-busy={busy}>
      {view.view === "empty" || view.view === "spread" ? (
        <div className="lists-bar">
          <PersonKeys
            owners={view.owners}
            selected={view.owner.id}
            everyone={false}
            label="Whose Binder"
            choose={(person) => choose({ person })}
          />
          {view.view === "spread" ? (
            <BinderValueBar value={view.value} />
          ) : null}
        </div>
      ) : null}
      {selected === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      <Body view={view} choose={choose} refresh={refresh} openEntry={setOpen} />
      <aside className="binder-credits" aria-label="OPTCG credits">
        <p>
          Card data and images come from the{" "}
          <a href="https://optcgapi.com" target="_blank" rel="noreferrer">
            OPTCG catalogue
          </a>
          .
        </p>
      </aside>
      <ListSheet
        open={selected !== undefined}
        close={() => setOpen(undefined)}
        title={selected?.card.name ?? "Card"}
      >
        {selected !== undefined && owner !== undefined ? (
          <>
            <BinderSheet
              key={selected.id}
              entry={selected}
              owner={owner}
              busy={busy}
              save={save}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
    </div>
  )
}

function Body({
  view,
  choose,
  refresh,
  openEntry,
}: {
  view: BinderView
  choose: (search: BinderSearch) => void
  refresh: () => Promise<void>
  openEntry: (id: string) => void
}): React.JSX.Element {
  switch (view.view) {
    case "unavailable":
      return (
        <div className="lists-empty">
          <CardsIcon size={48} weight="light" aria-hidden="true" />
          <h2>Binder unavailable</h2>
          <p>{binderFailureMessage[view.reason]}</p>
          <div className="binder-recovery">
            <CheckAgainButton check={refresh} />
            {view.reason === "unreachable" ||
            view.reason === "unavailable" ? null : (
              <OpenConnection />
            )}
          </div>
        </div>
      )
    case "no-household":
      return (
        <div className="lists-empty">
          <CardsIcon size={48} weight="light" aria-hidden="true" />
          <h2>No one here yet</h2>
          <p>Household members are added under Wish.</p>
          <Link to="/lists" search={{ tab: "wish" }} className="key-link-text">
            Open wishes
          </Link>
        </div>
      )
    case "empty":
      return (
        <div className="lists-empty">
          <CardsIcon size={48} weight="light" aria-hidden="true" />
          <h2>{view.owner.name}’s Binder is empty</h2>
          <p>Ask Hermes to add cards from a photo.</p>
        </div>
      )
    case "spread":
      return (
        <Spread
          left={view.left}
          right={view.right}
          index={view.index}
          count={view.count}
          owner={view.owner.id}
          choose={choose}
          openEntry={openEntry}
        />
      )
  }
}

function isEditing(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName))
  )
}

function Spread({
  left,
  right,
  index,
  count,
  owner,
  choose,
  openEntry,
}: {
  left: BinderEntry[]
  right: BinderEntry[]
  index: number
  count: number
  owner: string
  choose: (search: BinderSearch) => void
  openEntry: (id: string) => void
}): React.JSX.Element {
  const turn = (to: number): void => {
    if (to >= 0 && to < count) choose({ person: owner, page: to + 1 })
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (
        event.defaultPrevented ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        isEditing(event.target)
      )
        return

      if (event.key === "ArrowLeft") turn(index - 1)

      if (event.key === "ArrowRight") turn(index + 1)
    }

    window.addEventListener("keydown", onKey)

    return () => window.removeEventListener("keydown", onKey)
  })

  const first = index * 2 + 1

  return (
    <section className="binder-book" aria-label="Binder spread">
      <div className="binder-spread">
        <Page
          key={`${index}-left`}
          number={first}
          entries={left}
          openEntry={openEntry}
        />
        <Page
          key={`${index}-right`}
          number={first + 1}
          entries={right}
          openEntry={openEntry}
        />
      </div>
      <div className="binder-turn">
        <Button
          variant="outline"
          size="key-icon"
          disabled={index === 0}
          onClick={() => turn(index - 1)}
        >
          <CaretLeftIcon size={20} weight="bold" aria-hidden="true" />
          <span className="sr-only">Previous pages</span>
        </Button>
        <p className="binder-folio num" aria-live="polite">
          Pages {first}–{first + 1} of {count * 2}
        </p>
        <Button
          variant="outline"
          size="key-icon"
          disabled={index === count - 1}
          onClick={() => turn(index + 1)}
        >
          <CaretRightIcon size={20} weight="bold" aria-hidden="true" />
          <span className="sr-only">Next pages</span>
        </Button>
      </div>
    </section>
  )
}

/** One 3×3 page, filled to nine pockets so the rest show empty. */
function Page({
  number,
  entries,
  openEntry,
}: {
  number: number
  entries: BinderEntry[]
  openEntry: (id: string) => void
}): React.JSX.Element {
  const empty = Array.from(
    { length: pocketsPerPage - entries.length },
    (_, slot) => slot
  )

  return (
    <ol className="binder-page" aria-label={`Page ${number}`}>
      {entries.map((entry) => (
        <Pocket key={entry.id} entry={entry} open={() => openEntry(entry.id)} />
      ))}
      {empty.map((slot) => (
        <li
          key={`empty-${slot}`}
          className="pocket"
          data-empty=""
          aria-hidden="true"
        />
      ))}
    </ol>
  )
}

/** The card's own picture over its code and name, which stay if the picture cannot load. Tapping opens its sheet. */
function Pocket({
  entry,
  open,
}: {
  entry: BinderEntry
  open: () => void
}): React.JSX.Element {
  const { card, quantity, language } = entry

  return (
    <li className="pocket" data-language={language}>
      <button type="button" className="pocket-card" onClick={open}>
        <span className="pocket-type" aria-hidden="true">
          <span className="num">{card.card_code}</span>
          {card.name}
        </span>
        <img
          src={cardImageUrl(card.id)}
          alt=""
          width={600}
          height={838}
          loading="lazy"
          decoding="async"
          onError={(event) => {
            event.currentTarget.hidden = true
          }}
          onLoad={(event) => {
            event.currentTarget.hidden = false
          }}
        />
        <span className="sr-only">
          {card.name}, {card.card_code}
          {language === "ja" ? ", Japanese" : ""}
          {quantity > 1 ? `, ${quantity} copies` : ""}
        </span>
      </button>
      {language === "ja" ? (
        <span className="pocket-mark" aria-hidden="true">
          JP
        </span>
      ) : null}
      {quantity > 1 ? (
        <span className="pocket-quantity num" aria-hidden="true">
          ×{quantity}
        </span>
      ) : null}
    </li>
  )
}
