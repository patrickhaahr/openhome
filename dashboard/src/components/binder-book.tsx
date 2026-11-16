import { useEffect, useEffectEvent, useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
import { CardsIcon } from "@phosphor-icons/react/dist/csr/Cards"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { AddCard, type FindCards } from "@/components/binder-add"
import { CheckAgainButton } from "@/components/check-again-button"
import { OpenConnection } from "@/components/control-alert"
import { Page } from "@/components/binder-page"
import { BinderSheet } from "@/components/binder-sheet"
import { BinderValueBar } from "@/components/binder-value"
import { useReducedMotion } from "@/components/idle-screen"
import {
  ListBar,
  ListEmpty,
  ListsFrame,
  keyLink,
} from "@/components/list-frame"
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
  pocketsPerSpread,
  viewOf,
  type BinderEntry,
  type BinderSearch,
  type BinderState,
  type BinderView,
} from "@/lib/binder"
import type { PricesState } from "@/lib/card-prices"
import {
  grip,
  idle,
  nextTurn,
  opening,
  settleDuration,
  turnStep,
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
import type { BinderAddReceipt, BinderAddition } from "@/lib/binder-add"

/**
 * On a wide screen the bar's keys and value stand in the gutters beside the spread, which takes
 * the full height; elsewhere the credit shares the folio's line in the corner the turn keys leave.
 */
const frame =
  "tablet:relative binder-wide:grid-cols-binder-wide binder-wide:grid-rows-hug-fill-hug binder-wide:gap-x-8 binder-wide:*:data-[slot=lists-bar]:contents binder-wide:*:data-[slot=lists-empty]:col-start-2 binder-wide:*:data-[slot=lists-empty]:row-span-full binder-wide:[&>[role=status]]:col-start-3 binder-wide:[&>[role=status]]:row-start-2 binder-wide:[&>[role=status]]:text-right"

/**
 * One Household User's Binder, a spread of two facing 3×3 pages at a time in Card Code order.
 * The person keys choose whose Binder; dragging or tapping a page's edge, the arrows and the arrow
 * keys turn the spread, and a pocket opens its card sheet, whose arrows step to the neighbouring
 * cards and turn the spread with them. A read that failed offers a way to check again and never
 * shows an empty Binder.
 */
export function BinderBook({
  data,
  search,
  choose,
  change,
  refresh,
  prices,
  find,
  add,
}: {
  data: BinderState
  search: BinderSearch
  choose: (search: BinderSearch) => void
  change: (command: BinderCommand) => Promise<BinderMutation>
  refresh: () => Promise<void>
  prices: (card: string) => Promise<PricesState>
  find: FindCards
  add: (command: BinderAddition) => Promise<BinderAddReceipt>
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

  const order = view.view === "spread" ? view.entries : []
  const at = selected === undefined ? -1 : order.indexOf(selected)

  const stepCard = (offset: -1 | 1): void => {
    const to = at + offset
    const next = order[to]

    if (view.view !== "spread" || at === -1 || next === undefined) return
    setOpen(next.id)

    const spread = Math.floor(to / pocketsPerSpread)

    if (spread !== view.index)
      choose({ person: view.owner.id, page: spread + 1 })
  }

  return (
    <ListsFrame className={frame} aria-busy={busy}>
      {view.view === "empty" || view.view === "spread" ? (
        <ListBar>
          <PersonKeys
            owners={view.owners}
            selected={view.owner.id}
            everyone={false}
            label="Whose Binder"
            choose={(person) => choose({ person })}
            variant="rail"
          />
          <BinderValueBar value={view.value}>
            <AddCard
              owners={view.owners}
              person={view.owner.id}
              entries={data.state === "available" ? data.entries : []}
              find={find}
              change={add}
              refresh={refresh}
            />
          </BinderValueBar>
        </ListBar>
      ) : null}
      {selected === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      <Body view={view} choose={choose} refresh={refresh} openEntry={setOpen} />
      <aside
        className="text-label text-muted-ink tablet:absolute tablet:bottom-0 tablet:left-0 binder-wide:static binder-wide:col-start-3 binder-wide:row-start-3 binder-wide:justify-self-end binder-wide:text-right"
        aria-label="OPTCG credits"
      >
        <p>
          Card data and images come from the{" "}
          <a
            href="https://optcgapi.com"
            target="_blank"
            rel="noreferrer"
            className="underline"
          >
            OPTCG catalogue
          </a>
          .
        </p>
      </aside>
      <ListSheet
        open={selected !== undefined}
        close={() => setOpen(undefined)}
        title={selected?.card.name ?? "Card"}
        variant="stage"
        actions={
          at === -1 ? null : (
            <nav className="flex items-center gap-3" aria-label="Cards">
              <Button
                variant="outline"
                size="key-icon"
                disabled={at === 0}
                onClick={() => stepCard(-1)}
              >
                <CaretLeftIcon size={20} weight="bold" aria-hidden="true" />
                <span className="sr-only">Previous card</span>
              </Button>
              <p
                data-slot="binder-folio"
                className="min-w-18 text-center num text-muted-ink"
                aria-live="polite"
              >
                {at + 1} of {order.length}
              </p>
              <Button
                variant="outline"
                size="key-icon"
                disabled={at === order.length - 1}
                onClick={() => stepCard(1)}
              >
                <CaretRightIcon size={20} weight="bold" aria-hidden="true" />
                <span className="sr-only">Next card</span>
              </Button>
            </nav>
          )
        }
        onKeyDown={(event) => {
          if (
            event.defaultPrevented ||
            event.altKey ||
            event.ctrlKey ||
            event.metaKey ||
            isEditing(event.target)
          )
            return

          const offset =
            event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0

          if (offset === 0) return
          event.preventDefault()
          stepCard(offset)
        }}
      >
        {selected !== undefined && owner !== undefined ? (
          <>
            <BinderSheet
              key={selected.id}
              entry={selected}
              owner={owner}
              busy={busy}
              save={save}
              prices={prices}
              step={stepCard}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
    </ListsFrame>
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
        <ListEmpty icon={CardsIcon} title="Binder unavailable">
          <p>{binderFailureMessage[view.reason]}</p>
          <div className="flex items-center gap-3">
            <CheckAgainButton check={refresh} />
            {view.reason === "unreachable" ||
            view.reason === "unavailable" ? null : (
              <OpenConnection />
            )}
          </div>
        </ListEmpty>
      )
    case "no-household":
      return (
        <ListEmpty icon={CardsIcon} title="No one here yet">
          <p>Household members are added under Wish.</p>
          <Link to="/lists" search={{ tab: "wish" }} className={keyLink}>
            Open wishes
          </Link>
        </ListEmpty>
      )
    case "empty":
      return (
        <ListEmpty
          icon={CardsIcon}
          title={<>{view.owner.name}’s Binder is empty</>}
        >
          <p>
            Ask Hermes to add cards from a photo, or add one by its Card Code.
          </p>
        </ListEmpty>
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
    <section
      className="grid grid-cols-flanked items-center justify-items-center gap-x-binder-turn-gap gap-y-2 max-tablet:grid-cols-binder-turn max-tablet:gap-x-6 binder-wide:col-start-2 binder-wide:row-span-full binder-wide:gap-x-4"
      aria-label="Binder spread"
    >
      <Button
        variant="outline"
        size="key-icon"
        className="col-start-1 row-start-1 max-tablet:row-start-2 max-tablet:justify-self-end"
        disabled={!can.back}
        onClick={() => flip("back")}
      >
        <CaretLeftIcon size={20} weight="bold" aria-hidden="true" />
        <span className="sr-only">Previous pages</span>
      </Button>
      <div
        ref={spread}
        data-slot="binder-spread"
        className="col-start-2 row-start-1 grid binder-spread touch-pan-y grid-cols-2 gap-(--spine-gap) select-none touch-callout-none max-tablet:col-span-full"
        onPointerDown={(event) => {
          if (!event.isPrimary || event.button !== 0) return

          step({
            type: "press",
            pointer: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            time: event.timeStamp,
            pocket:
              event.target instanceof Element &&
              event.target.closest('[data-slot="pocket-card"]') !== null,
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
              data-slot="binder-side"
              className="relative min-w-0 data-lifted:z-1"
              data-side={side}
              data-lifted={lifted === undefined ? undefined : ""}
            >
              {lifted === undefined ? null : (
                <Page sheet={lifted.under} openEntry={openEntry} under />
              )}
              <div
                ref={lifted === undefined ? undefined : leaf}
                data-slot="binder-leaf"
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
      <Button
        variant="outline"
        size="key-icon"
        className="col-start-3 row-start-1 max-tablet:row-start-2 max-tablet:justify-self-start"
        disabled={!can.forward}
        onClick={() => flip("forward")}
      >
        <CaretRightIcon size={20} weight="bold" aria-hidden="true" />
        <span className="sr-only">Next pages</span>
      </Button>
      <p
        data-slot="binder-folio"
        className="col-start-2 row-start-2 min-w-28 text-center text-label num text-muted-ink"
        aria-live="polite"
      >
        Pages {first}–{first + 1} of {count * 2}
      </p>
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
