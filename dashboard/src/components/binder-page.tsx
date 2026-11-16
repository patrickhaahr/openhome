import { cardImageUrl, pocketsPerPage, type BinderEntry } from "@/lib/binder"
import type { Sheet } from "@/lib/binder-turn"
import { cn } from "@/lib/utils"

const tag =
  "pointer-events-none absolute bottom-1.5 rounded-full px-2 py-0.5 text-label leading-5 font-bold"

const pocket =
  "relative aspect-binder-card rounded-code bg-wash-strong data-empty:bg-wash"

/** One 3×3 page, filled to nine pockets so the rest show empty; `under` lies beneath a lifted leaf. */
export function Page({
  sheet,
  openEntry,
  under = false,
}: {
  sheet: Sheet<BinderEntry>
  openEntry: (id: string) => void
  under?: boolean
}): React.JSX.Element {
  const { number, entries } = sheet

  const empty = Array.from(
    { length: pocketsPerPage - entries.length },
    (_, slot) => slot
  )

  return (
    <ol
      data-slot="binder-page"
      className={cn(
        "grid min-w-0 grid-cols-3 gap-(--pocket-gap)",
        under && "absolute inset-0"
      )}
      aria-label={`Page ${number}`}
    >
      {entries.map((entry) => (
        <Pocket key={entry.id} entry={entry} open={() => openEntry(entry.id)} />
      ))}
      {empty.map((slot) => (
        <li
          key={`empty-${slot}`}
          data-slot="pocket"
          className={pocket}
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
    <li data-slot="pocket" className={pocket} data-language={language}>
      <button
        type="button"
        data-slot="pocket-card"
        className="absolute inset-0 block overflow-hidden rounded-code text-left"
        onClick={open}
      >
        <span
          className="absolute inset-0 flex flex-col justify-end gap-1 p-2 text-label leading-none font-semibold tracking-figure-tight text-balance text-ink max-tablet:p-1.5 max-tablet:text-micro"
          aria-hidden="true"
        >
          <span className="num">{card.card_code}</span>
          {card.name}
        </span>
        <img
          className="absolute inset-0 size-full object-cover"
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
        <span
          data-slot="pocket-mark"
          className={cn(tag, "left-1.5 bg-sun text-binder-mark")}
          aria-hidden="true"
        >
          JP
        </span>
      ) : null}
      {quantity > 1 ? (
        <span
          data-slot="pocket-quantity"
          className={cn(tag, "right-1.5 bg-ink num text-ground")}
          aria-hidden="true"
        >
          ×{quantity}
        </span>
      ) : null}
    </li>
  )
}
