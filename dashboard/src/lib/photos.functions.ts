import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { chooseView, galleryInput, wallInput } from "./photos"

/**
 * Server-only album and first-page reads; no Immich configuration reaches the browser. Without
 * a chosen album the page shows the albums, or opens the only one when just one is configured.
 */
export const getGallery = createServerFn({ method: "GET" })
  .validator(galleryInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readAlbums, readWall } = await import("./photos.server")
    const albums = await readAlbums()

    if (albums.state !== "ready") return { albums, view: null, wall: null }
    const view = chooseView(albums.albums, data.album)

    return {
      albums,
      view: view ?? null,
      wall: view === undefined ? null : await readWall(view, 0),
    }
  })

/** A later page of the open wall, read as it scrolls. */
export const getWallPage = createServerFn({ method: "GET" })
  .validator(wallInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWall } = await import("./photos.server")

    return readWall(data.album, data.page)
  })
