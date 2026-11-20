import { useState } from "react"
import { cva } from "class-variance-authority"
import { GearSixIcon } from "@phosphor-icons/react/dist/csr/GearSix"
import { GiftIcon } from "@phosphor-icons/react/dist/csr/Gift"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { OpenConnection } from "@/components/control-alert"
import { HouseholdLists } from "@/components/household-sheet"
import {
  BoardHeading,
  ListBar,
  ListBarEnd,
  ListEmpty,
  ListsFrame,
  WishMark,
  listKey,
  listKeys,
} from "@/components/list-frame"
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
      <ListsFrame>
        {lead === undefined ? null : <ListBar>{lead}</ListBar>}
        <ListEmpty icon={GiftIcon} title="Wishlists unavailable">
          <p>{wishlistFailureMessage[data.reason]}</p>
          <OpenConnection />
        </ListEmpty>
      </ListsFrame>
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
              <PersonDisc name={person.name} size="heading" />
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
    <ListsFrame aria-busy={busy}>
      <ListBar>
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
        <ListBarEnd>
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
        </ListBarEnd>
      </ListBar>
      {owner === undefined ? null : (
        <div
          className={listKeys}
          role="group"
          aria-label={`${owner.name}’s lists`}
        >
          <button
            type="button"
            className={listKey()}
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
                className={listKey()}
                aria-pressed={list?.id === candidate.id}
                onClick={() => choose({ person: owner.id, list: candidate.id })}
              >
                {candidate.name}
                <span className="text-label font-medium num text-muted-ink">
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
        <ListEmpty icon={GiftIcon} title="No one here yet">
          <p>Add a household member, then their first list.</p>
          <Button
            variant="outline"
            onClick={() => setOpen({ kind: "household" })}
          >
            Add a household member
          </Button>
        </ListEmpty>
      ) : (
        <div
          className="group/board grid gap-board"
          data-density={shown <= 3 ? "few" : shown <= 8 ? "some" : "many"}
        >
          {groups.map((group) => (
            <section
              key={group.key}
              className="grid gap-3.5"
              aria-label={group.label}
            >
              <BoardHeading
                count={group.items.length}
                noun={["wish", "wishes"]}
                hidden={group.hidden}
              >
                {group.heading}
              </BoardHeading>
              <ul className="grid grid-fill-50 gap-plates group-data-[density=few]/board:grid-fill-72 group-data-[density=some]/board:grid-fill-58 max-sm:grid-fill-38">
                {group.items.map((item) => (
                  <li key={item.id} className="grid">
                    <WishPlate
                      item={item}
                      image={imageOf(item)}
                      caption={group.caption ? listOf(item)?.name : undefined}
                      open={() => setOpen({ kind: "wish", id: item.id })}
                    />
                  </li>
                ))}
                {group.items.length === 0 ? (
                  <li className="grid">
                    <button
                      type="button"
                      data-slot="wish-plate"
                      className={wishPlate({ kind: "add" })}
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
    </ListsFrame>
  )
}

const wishPlate = cva(
  "group/plate relative grid w-full overflow-hidden rounded-plate text-left transition duration-fast ease-app hover:bg-wash-strong active:scale-98",
  {
    variants: {
      kind: {
        wish: "grid-rows-art bg-tile",
        add: "min-h-48 grid-rows-none place-items-center content-center gap-2 bg-wash font-medium text-muted-ink",
      },
    },
    defaultVariants: { kind: "wish" },
  }
)

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
      data-slot="wish-plate"
      className={wishPlate()}
      data-priority={item.priority}
      onClick={open}
    >
      <WishArt title={item.title} image={image} />
      {item.priority === "high" ? <WishMark /> : null}
      <span className="grid content-start gap-stack px-3.5 pt-3 pb-3.5">
        <span
          data-slot="wish-title"
          className="line-clamp-2 text-base leading-tight font-medium tracking-nav group-data-[priority=low]/plate:text-muted-ink"
        >
          {item.title}
        </span>
        <span className="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-control text-muted-ink">
          {item.price === null ? null : (
            <span className="font-semibold num text-ink">{item.price}</span>
          )}
          {item.quantity > 1 ? (
            <span className="num">
              <span aria-hidden="true">×</span>
              {item.quantity}
              <span className="sr-only"> wanted</span>
            </span>
          ) : null}
          {caption === undefined ? null : (
            <span className="ml-auto text-label">{caption}</span>
          )}
        </span>
        <span className="sr-only">
          {item.priority === "normal" ? "" : `, ${item.priority} priority`}
        </span>
      </span>
    </button>
  )
}
