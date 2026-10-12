import type { WishlistItem } from "@/lib/wishlist"

/** Stored context with opt-in image loading to avoid sending household display visits to remote hosts. */
export function ItemDetails({
  item,
}: {
  item: WishlistItem
}): React.JSX.Element {
  return (
    <div className="wishlist-item-title">
      <strong>{item.title}</strong>
      <div className="wishlist-item-meta">
        <span>Qty {item.quantity}</span>
        <span data-priority={item.priority}>{item.priority} priority</span>
        {item.price !== null ? <span>{item.price}</span> : null}
      </div>
      {item.notes !== null ? (
        <p className="wishlist-item-notes">{item.notes}</p>
      ) : null}
      {item.url !== null || item.image_url !== null ? (
        <div className="wishlist-item-links">
          {item.url !== null ? (
            <a href={item.url} target="_blank" rel="noreferrer">
              Product link ↗
            </a>
          ) : null}
          {item.image_url !== null ? (
            <a href={item.image_url} target="_blank" rel="noreferrer">
              View image ↗
            </a>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}
