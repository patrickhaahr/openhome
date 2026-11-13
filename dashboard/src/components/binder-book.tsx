import { useEffect } from "react"
import { Link } from "@tanstack/react-router"
import { CardsIcon } from "@phosphor-icons/react/dist/csr/Cards"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { AddCard, type FindCards } from "@/components/binder-add"
import { CheckAgainButton } from "@/components/check-again-button"
import { OpenConnection } from "@/components/control-alert"
import { PersonKeys } from "@/components/list-parts"
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
import type { BinderAddReceipt, BinderAddition } from "@/lib/binder-add"

/**
 * One Household User's Binder, a spread of two facing 3×3 pages at a time in Card Code order.
 * The person keys choose whose Binder; the arrows and the arrow keys turn the spread. A read
 * that failed offers a way to check again and never shows an empty Binder.
 */
export function BinderBook({
  data,
  search,
  choose,
  refresh,
  find,
  change,
}: {
  data: BinderState
  search: BinderSearch
  choose: (search: BinderSearch) => void
  refresh: () => Promise<void>
  find: FindCards
  change: (command: BinderAddition) => Promise<BinderAddReceipt>
}): React.JSX.Element {
  const view = viewOf(data, search)

  return (
    <div className="lists binder">
      {view.view === "empty" || view.view === "spread" ? (
        <div className="lists-bar">
          <PersonKeys
            owners={view.owners}
            selected={view.owner.id}
            everyone={false}
            label="Whose Binder"
            choose={(person) => choose({ person })}
          />
          <div className="lists-bar-end">
            <AddCard
              owners={view.owners}
              person={view.owner.id}
              entries={data.state === "available" ? data.entries : []}
              find={find}
              change={change}
              refresh={refresh}
            />
          </div>
        </div>
      ) : null}
      <Body view={view} choose={choose} refresh={refresh} />
      <aside className="binder-credits" aria-label="OPTCG credits">
        <p>
          Card data and images come from the{" "}
          <a href="https://optcgapi.com" target="_blank" rel="noreferrer">
            OPTCG catalogue
          </a>
          .
        </p>
      </aside>
    </div>
  )
}

function Body({
  view,
  choose,
  refresh,
}: {
  view: BinderView
  choose: (search: BinderSearch) => void
  refresh: () => Promise<void>
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
          <p>
            Ask Hermes to add cards from a photo, or add one by its Card Code.
          </p>
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
}: {
  left: BinderEntry[]
  right: BinderEntry[]
  index: number
  count: number
  owner: string
  choose: (search: BinderSearch) => void
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
        <Page key={`${index}-left`} number={first} entries={left} />
        <Page key={`${index}-right`} number={first + 1} entries={right} />
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
}: {
  number: number
  entries: BinderEntry[]
}): React.JSX.Element {
  const empty = Array.from(
    { length: pocketsPerPage - entries.length },
    (_, slot) => slot
  )

  return (
    <ol className="binder-page" aria-label={`Page ${number}`}>
      {entries.map((entry) => (
        <Pocket key={entry.id} entry={entry} />
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

/** The card's own picture over its code and name, which stay if the picture cannot load. */
function Pocket({ entry }: { entry: BinderEntry }): React.JSX.Element {
  const { card, quantity, language } = entry

  return (
    <li className="pocket" data-language={language}>
      <span className="pocket-card">
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
      </span>
      <span className="sr-only">
        {card.name}, {card.card_code}
        {language === "ja" ? ", Japanese" : ""}
        {quantity > 1 ? `, ${quantity} copies` : ""}
      </span>
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
