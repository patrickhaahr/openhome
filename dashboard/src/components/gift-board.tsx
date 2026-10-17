import { useState } from "react"
import { GearSixIcon } from "@phosphor-icons/react/dist/csr/GearSix"
import { GiftIcon } from "@phosphor-icons/react/dist/csr/Gift"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { OpenConnection } from "@/components/control-alert"
import { HouseholdLists } from "@/components/household-sheet"
import {
  FeedbackLine,
  ListSheet,
  PersonDisc,
  PersonKeys,
  useListSave,
} from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import { WishAdd } from "@/components/wish-add"
import { WishArt, WishDetail } from "@/components/wish-sheet"
import type { WishSearch } from "@/lib/lists"
import {
  wishImageUrl,
  wishlistFailureMessage,
  type HouseholdUser,
  type Wishlist,
  type WishlistCommand,
  type WishlistItem,
  type WishlistMetadataLoader,
  type WishlistMutation,
  type WishlistState,
} from "@/lib/wishlist"

const rank = { high: 0, normal: 1, low: 2 } as const

/** What to buy first: high priority, then normal, then low, each in the order they were wished. */
function byPriority(items: readonly WishlistItem[]): WishlistItem[] {
  return items
    .map((item, index) => ({ item, index }))
    .sort(
      (a, b) =>
        rank[a.item.priority] - rank[b.item.priority] || a.index - b.index
    )
    .map(({ item }) => item)
}

type Open =
  | { kind: "wish"; id: string }
  | { kind: "add"; person: string | undefined; list: string | undefined }
  | { kind: "household" }

/**
 * The gift board: every wish as an image-led plate, by person and by named list, so the board
 * answers "what can I buy for them?". Tapping a plate opens its detail plate.
 */
export function GiftBoard({
  data,
  search,
  choose,
  change,
  metadata,
  refresh,
  lead,
}: {
  data: WishlistState
  search: WishSearch
  choose: (search: WishSearch) => void
  change: (command: WishlistCommand) => Promise<WishlistMutation>
  metadata: WishlistMetadataLoader
  refresh: () => Promise<void>
  lead?: React.ReactNode
}): React.JSX.Element {
  const { busy, feedback, save } = useListSave(
    change,
    refresh,
    wishlistFailureMessage,
    "unreachable"
  )

  const [open, setOpen] = useState<Open>()

  if (data.state === "unavailable")
    return (
      <div className="lists">
        {lead === undefined ? null : <div className="lists-bar">{lead}</div>}
        <div className="lists-empty">
          <GiftIcon size={48} weight="light" aria-hidden="true" />
          <h2>Wishlists unavailable</h2>
          <p>{wishlistFailureMessage[data.reason]}</p>
          <OpenConnection />
        </div>
      </div>
    )

  const { owners, lists, items } = data
  const owner = owners.find((candidate) => candidate.id === search.person)

  const list = lists.find(
    (candidate) =>
      candidate.id === search.list && candidate.owner_id === owner?.id
  )

  const listCounts = new Map<string, number>()

  for (const item of items)
    listCounts.set(item.list_id, (listCounts.get(item.list_id) ?? 0) + 1)

  const ownerCounts = new Map(
    owners.map((person) => [
      person.id,
      lists
        .filter((candidate) => candidate.owner_id === person.id)
        .reduce(
          (sum, candidate) => sum + (listCounts.get(candidate.id) ?? 0),
          0
        ),
    ])
  )

  const itemsIn = (target: Wishlist): WishlistItem[] =>
    byPriority(items.filter((item) => item.list_id === target.id))

  const listOf = (item: WishlistItem): Wishlist | undefined =>
    lists.find((candidate) => candidate.id === item.list_id)

  const imageOf = (item: WishlistItem): string | null => {
    const owner = listOf(item)?.owner_id

    return owner === undefined ? null : wishImageUrl(owner, item)
  }

  const ownerOf = (target: Wishlist): HouseholdUser | undefined =>
    owners.find((candidate) => candidate.id === target.owner_id)

  // Groups shown: per person for everyone, per list for one person, one list alone.
  const groups: Array<{
    key: string
    heading: React.ReactNode
    label: string
    hidden: boolean
    items: WishlistItem[]
    caption: boolean
    add: { person: string; list: string | undefined }
  }> =
    owner === undefined
      ? owners.map((person) => ({
          key: person.id,
          label: person.name,
          heading: (
            <>
              <PersonDisc name={person.name} />
              {person.name}
            </>
          ),
          hidden: false,
          items: byPriority(
            items.filter((item) => listOf(item)?.owner_id === person.id)
          ),
          caption: true,
          add: { person: person.id, list: undefined },
        }))
      : (list === undefined
          ? lists.filter((candidate) => candidate.owner_id === owner.id)
          : [list]
        ).map((target) => ({
          key: target.id,
          label: target.name,
          heading: target.name,
          hidden: list !== undefined,
          items: itemsIn(target),
          caption: false,
          add: { person: owner.id, list: target.id },
        }))

  const shown = groups.reduce((sum, group) => sum + group.items.length, 0)

  const selected =
    open?.kind === "wish"
      ? items.find((item) => item.id === open.id)
      : undefined

  const selectedList = selected === undefined ? undefined : listOf(selected)

  const selectedOwner =
    selectedList === undefined ? undefined : ownerOf(selectedList)

  return (
    <div className="lists" aria-busy={busy}>
      <div className="lists-bar">
        {lead}
        {owners.length === 0 ? null : (
          <PersonKeys
            owners={owners}
            selected={owner?.id}
            counts={ownerCounts}
            noun={["wish", "wishes"]}
            choose={(person) => choose({ person, list: undefined })}
          />
        )}
        <div className="lists-bar-end">
          <Button
            variant="outline"
            size="key-icon"
            title="Household and lists"
            onClick={() => setOpen({ kind: "household" })}
          >
            <GearSixIcon size={20} aria-hidden="true" />
            <span className="sr-only">Household and lists</span>
          </Button>
          <Button
            size="key"
            disabled={owners.length === 0}
            onClick={() =>
              setOpen({ kind: "add", person: owner?.id, list: list?.id })
            }
          >
            <PlusIcon size={20} weight="bold" aria-hidden="true" />
            Add a wish
          </Button>
        </div>
      </div>
      {owner === undefined ? null : (
        <div
          className="list-keys"
          role="group"
          aria-label={`${owner.name}’s lists`}
        >
          <button
            type="button"
            className="list-key"
            aria-pressed={list === undefined}
            onClick={() => choose({ person: owner.id, list: undefined })}
          >
            All lists
          </button>
          {lists
            .filter((candidate) => candidate.owner_id === owner.id)
            .map((candidate) => (
              <button
                key={candidate.id}
                type="button"
                className="list-key"
                aria-pressed={list?.id === candidate.id}
                onClick={() => choose({ person: owner.id, list: candidate.id })}
              >
                {candidate.name}
                <span className="num">
                  {listCounts.get(candidate.id) ?? 0}
                  <span className="sr-only"> wishes</span>
                </span>
              </button>
            ))}
        </div>
      )}
      {open === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      {owners.length === 0 ? (
        <div className="lists-empty">
          <GiftIcon size={48} weight="light" aria-hidden="true" />
          <h2>No one here yet</h2>
          <p>Add a household member, then their first list.</p>
          <Button
            variant="outline"
            onClick={() => setOpen({ kind: "household" })}
          >
            Add a household member
          </Button>
        </div>
      ) : (
        <div
          className="board"
          data-density={shown <= 3 ? "few" : shown <= 8 ? "some" : "many"}
        >
          {groups.map((group) => (
            <section
              key={group.key}
              className="board-group"
              aria-label={group.label}
            >
              <h2 className={group.hidden ? "sr-only" : "board-heading"}>
                {group.heading}
                <span className="board-count num">
                  {group.items.length}
                  <span className="sr-only">
                    {" "}
                    {group.items.length === 1 ? "wish" : "wishes"}
                  </span>
                </span>
              </h2>
              <ul className="plates">
                {group.items.map((item) => (
                  <li key={item.id}>
                    <WishPlate
                      item={item}
                      image={imageOf(item)}
                      caption={group.caption ? listOf(item)?.name : undefined}
                      open={() => setOpen({ kind: "wish", id: item.id })}
                    />
                  </li>
                ))}
                {group.items.length === 0 ? (
                  <li>
                    <button
                      type="button"
                      className="wish-plate wish-plate-add"
                      onClick={() => setOpen({ kind: "add", ...group.add })}
                    >
                      <PlusIcon size={28} aria-hidden="true" />
                      <span>Add the first wish</span>
                    </button>
                  </li>
                ) : null}
              </ul>
            </section>
          ))}
        </div>
      )}
      <ListSheet
        open={selected !== undefined}
        close={() => setOpen(undefined)}
        title={selected?.title ?? "Wish"}
        titleHidden
      >
        {selected !== undefined &&
        selectedList !== undefined &&
        selectedOwner !== undefined ? (
          <>
            <WishDetail
              key={selected.id}
              item={selected}
              list={selectedList}
              owner={selectedOwner}
              busy={busy}
              save={save}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
      <ListSheet
        open={open?.kind === "add"}
        close={() => setOpen(undefined)}
        title="New wish"
      >
        {open?.kind === "add" ? (
          <>
            <WishAdd
              owners={owners}
              lists={lists}
              person={open.person}
              list={open.list}
              busy={busy}
              save={save}
              metadata={metadata}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
      <ListSheet
        open={open?.kind === "household"}
        close={() => setOpen(undefined)}
        title="Household and lists"
      >
        <HouseholdLists
          owners={owners}
          lists={lists}
          counts={listCounts}
          busy={busy}
          save={save}
        />
        <FeedbackLine feedback={feedback} busy={busy} />
      </ListSheet>
    </div>
  )
}

/** One wish: the image fills the plate; title, price and quantity sit beneath it. */
function WishPlate({
  item,
  image,
  caption,
  open,
}: {
  item: WishlistItem
  image: string | null
  caption: string | undefined
  open: () => void
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="wish-plate"
      data-priority={item.priority}
      onClick={open}
    >
      <WishArt title={item.title} image={image} />
      {item.priority === "high" ? (
        <i className="wish-mark" aria-hidden="true" />
      ) : null}
      <span className="wish-text">
        <span className="wish-title">{item.title}</span>
        <span className="wish-meta">
          {item.price === null ? null : (
            <span className="num">{item.price}</span>
          )}
          {item.quantity > 1 ? (
            <span className="num wish-quantity">
              <span aria-hidden="true">×</span>
              {item.quantity}
              <span className="sr-only"> wanted</span>
            </span>
          ) : null}
          {caption === undefined ? null : (
            <span className="wish-caption">{caption}</span>
          )}
        </span>
        <span className="sr-only">
          {item.priority === "normal" ? "" : `, ${item.priority} priority`}
        </span>
      </span>
    </button>
  )
}
