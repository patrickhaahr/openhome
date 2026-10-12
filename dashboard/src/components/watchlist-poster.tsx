import { posterUrl } from "@/lib/watchlist"

/** Reset failed images for a replacement URL and reveal later successful loads. */
export function WatchlistPoster({ path }: { path: string }): React.JSX.Element {
  return (
    <img
      key={path}
      className="watchlist-poster"
      src={posterUrl(path)}
      alt=""
      width={68}
      height={102}
      loading="lazy"
      onError={(event) => {
        event.currentTarget.hidden = true
      }}
      onLoad={(event) => {
        event.currentTarget.hidden = false
      }}
    />
  )
}
