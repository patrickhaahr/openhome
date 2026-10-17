import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { chooseView, galleryInput, onThisDayInput, wallInput } from "./photos"

/**
 * Server-only album and first-page reads; no Immich configuration reaches the browser. Without
 * a chosen album the page shows the albums, or opens the only one when just one is configured.
 */
export const getGallery = createServerFn({ method: "GET" })
  .validator(galleryInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")

    const { readAlbums, readOnThisDay, readWall } =
      await import("./photos.server")

    const albums = await readAlbums()

    if (albums.state !== "ready")
      return { albums, view: null, wall: null, onThisDay: null }
    const view = chooseView(albums.albums, data.album)

    // The albums offer today's photos from earlier years when there are any.
    return view === undefined
      ? {
          albums,
          view: null,
          wall: null,
          onThisDay: await readOnThisDay(undefined, 0),
        }
      : { albums, view, wall: await readWall(view, 0), onThisDay: null }
  })

/**
 * Photos taken on a month-day (`MM-DD`; today in Copenhagen when omitted) in earlier years,
 * from the selected albums, newest year first, 60 to a page. Each photo names the album that
 * grants access, for `photoUrl`. An unavailable result never means there are none.
 */
export const getOnThisDay = createServerFn({ method: "GET" })
  .validator(onThisDayInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readOnThisDay } = await import("./photos.server")

    return readOnThisDay(data.date, data.page)
  })

/** A later page of the open wall, read as it scrolls. */
export const getWallPage = createServerFn({ method: "GET" })
  .validator(wallInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWall } = await import("./photos.server")

    return readWall(data.album, data.page)
  })

/** The selected albums alone, for the Idle Slideshow to deal its pages from. */
export const getSlideshowAlbums = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readAlbums } = await import("./photos.server")

    return readAlbums()
  }
)
