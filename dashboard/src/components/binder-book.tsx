import { useEffect, useEffectEvent, useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
import { CardsIcon } from "@phosphor-icons/react/dist/csr/Cards"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { CheckAgainButton } from "@/components/check-again-button"
import { OpenConnection } from "@/components/control-alert"
import { BinderSheet } from "@/components/binder-sheet"
import { useReducedMotion } from "@/components/idle-screen"
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
  paginate,
  pocketsPerPage,
  viewOf,
  type BinderEntry,
  type BinderSearch,
  type BinderState,
  type BinderView,
} from "@/lib/binder"
import {
  grip,
  idle,
  nextTurn,
  opening,
  settleDuration,
  turnStep,
  type Sheet,
  type Turn,
  type TurnDirection,
  type TurnEvent,
  type TurnOutcome,
} from "@/lib/binder-turn"
import {
  binderWriteMessage,
  type BinderCommand,
  type BinderMutation,
} from "@/lib/binder-edit"

/**
 * One Household User's Binder, a spread of two facing 3×3 pages at a time in Card Code order.
 * The person keys choose whose Binder; dragging or tapping a page's edge, the arrows and the arrow
 * keys turn the spread, and a pocket opens its card sheet. A read that failed offers a way to check again and never shows
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
          key={`${view.owner.id}-${view.index}`}
          entries={view.entries}
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

function isGesture(turn: Turn): boolean {
  return turn.phase === "dragging" || turn.phase === "refused"
}

function Spread({
  entries,
  index,
  count,
  owner,
  choose,
  openEntry,
}: {
  entries: BinderEntry[]
  index: number
  count: number
  owner: string
  choose: (search: BinderSearch) => void
  openEntry: (id: string) => void
}): React.JSX.Element {
  const reduced = useReducedMotion()
  const turn = useRef<Turn>(idle)
  const [shown, setShown] = useState<Turn>(idle)
  const spread = useRef<HTMLDivElement>(null)
  const leaf = useRef<HTMLDivElement>(null)
  const progress = useRef(0)
  const frame = useRef(0)
  const dragged = useRef(false)

  const can = { back: index > 0, forward: index < count - 1 }

  const draw = (): void => {
    frame.current = 0
    const current = turn.current

    if (leaf.current === null || !("direction" in current)) return
    leaf.current.style.transform = `perspective(250rem) rotateY(${progress.current * turnStep[current.direction] * -180}deg)`
  }

  const step = (event: TurnEvent): void => {
    const before = turn.current
    const next = nextTurn(before, event)

    const after =
      reduced && next.phase === "settling"
        ? nextTurn(next, { type: "settled" })
        : next

    if (after === before) return
    turn.current = after

    if (isGesture(after) && !isGesture(before)) dragged.current = true

    // The click a drag's release produces arrives in the same task; one that never comes must not swallow a later one.
    if (isGesture(before) && !isGesture(after))
      requestAnimationFrame(() => {
        dragged.current = false
      })

    if (after.phase === "dragging") {
      if (before.phase === "pressing")
        spread.current?.setPointerCapture(after.pointer)
      progress.current = after.progress

      if (frame.current === 0) frame.current = requestAnimationFrame(draw)
    }

    if (after.phase === "turned")
      choose({ person: owner, page: index + turnStep[after.direction] + 1 })

    if (after.phase === "idle" && leaf.current !== null)
      leaf.current.style.transform = ""

    if (after.phase !== before.phase) setShown(after)
  }

  const flip = (direction: TurnDirection): void => {
    if (can[direction]) step({ type: "flip", direction })
  }

  const land = useEffectEvent((outcome: TurnOutcome): void => {
    cancelAnimationFrame(frame.current)
    progress.current = outcome === "complete" ? 1 : 0
    draw()
  })

  const settled = useEffectEvent((): void => step({ type: "settled" }))

  useEffect(() => {
    if (shown.phase !== "settling") return
    land(shown.outcome)

    const timer = setTimeout(settled, settleDuration)

    return () => clearTimeout(timer)
  }, [shown])

  usePrefetchNeighbours(entries, index, count)

  useEffect(() => () => cancelAnimationFrame(frame.current), [])

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

      if (event.key === "ArrowLeft") flip("back")

      if (event.key === "ArrowRight") flip("forward")
    }

    window.addEventListener("keydown", onKey)

    return () => window.removeEventListener("keydown", onKey)
  })

  const first = index * 2 + 1

  const open = opening(
    entries,
    index,
    reduced || !("direction" in shown) ? undefined : shown.direction
  )

  return (
    <section className="binder-book" aria-label="Binder spread">
      <div
        ref={spread}
        className="binder-spread"
        onPointerDown={(event) => {
          if (!event.isPrimary || event.button !== 0) return

          step({
            type: "press",
            pointer: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            pocket:
              event.target instanceof Element &&
              event.target.closest(".pocket-card") !== null,
            grip: grip(
              event.currentTarget.getBoundingClientRect(),
              event.clientX,
              can
            ),
          })
        }}
        onPointerMove={(event) =>
          step({
            type: "move",
            pointer: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            time: event.timeStamp,
          })
        }
        onPointerUp={(event) =>
          step({
            type: "release",
            pointer: event.pointerId,
            x: event.clientX,
            time: event.timeStamp,
          })
        }
        onPointerCancel={(event) =>
          step({ type: "lose", pointer: event.pointerId })
        }
        onLostPointerCapture={(event) =>
          step({ type: "lose", pointer: event.pointerId })
        }
        onPointerLeave={(event) =>
          step({ type: "lose", pointer: event.pointerId })
        }
        onClickCapture={(event) => {
          if (!dragged.current) return
          dragged.current = false
          event.preventDefault()
          event.stopPropagation()
        }}
      >
        {(["left", "right"] as const).map((side) => {
          const lifted = open.lifted?.side === side ? open.lifted : undefined

          return (
            <div
              key={side}
              className="binder-side"
              data-side={side}
              data-lifted={lifted === undefined ? undefined : ""}
            >
              {lifted === undefined ? null : (
                <Page sheet={lifted.under} openEntry={openEntry} />
              )}
              <div
                ref={lifted === undefined ? undefined : leaf}
                className="binder-leaf"
                data-settling={
                  lifted !== undefined && shown.phase === "settling"
                    ? ""
                    : undefined
                }
                // SAFETY: CSSProperties omits custom properties; --settle is a CSS duration.
                style={
                  { "--settle": `${settleDuration}ms` } as React.CSSProperties
                }
              >
                <Page sheet={open[side]} openEntry={openEntry} />
                {lifted === undefined ? null : (
                  <Page sheet={lifted.back} openEntry={openEntry} />
                )}
              </div>
            </div>
          )
        })}
      </div>
      <div className="binder-turn">
        <Button
          variant="outline"
          size="key-icon"
          disabled={!can.back}
          onClick={() => flip("back")}
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
          disabled={!can.forward}
          onClick={() => flip("forward")}
        >
          <CaretRightIcon size={20} weight="bold" aria-hidden="true" />
          <span className="sr-only">Next pages</span>
        </Button>
      </div>
    </section>
  )
}

function usePrefetchNeighbours(
  entries: BinderEntry[],
  index: number,
  count: number
): void {
  useEffect(() => {
    for (const to of [index - 1, index + 1]) {
      if (to < 0 || to >= count) continue

      const { left, right } = paginate(entries, to)

      for (const entry of [...left, ...right])
        new Image().src = cardImageUrl(entry.card.id)
    }
  }, [entries, index, count])
}

/** One 3×3 page, filled to nine pockets so the rest show empty. */
function Page({
  sheet,
  openEntry,
}: {
  sheet: Sheet<BinderEntry>
  openEntry: (id: string) => void
}): React.JSX.Element {
  const { number, entries } = sheet

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
          draggable={false}
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
