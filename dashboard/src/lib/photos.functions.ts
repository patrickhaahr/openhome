import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { chooseView, galleryInput, onThisDayInput, wallInput } from "./photos"

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

    return view === undefined
      ? {
          albums,
          view: null,
          wall: null,
          onThisDay: await readOnThisDay(undefined, 0),
        }
      : { albums, view, wall: await readWall(view, 0), onThisDay: null }
  })

export const getOnThisDay = createServerFn({ method: "GET" })
  .validator(onThisDayInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readOnThisDay } = await import("./photos.server")

    return readOnThisDay(data.date, data.page)
  })

export const getWallPage = createServerFn({ method: "GET" })
  .validator(wallInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWall } = await import("./photos.server")

    return readWall(data.album, data.page)
  })

export const getSlideshowAlbums = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readAlbums } = await import("./photos.server")

    return readAlbums()
  }
)
