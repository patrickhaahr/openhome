import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { photoSearch } from "./photos"

/** Server-only album and page reads; no Immich configuration reaches the browser. */
export const getGallery = createServerFn({ method: "GET" })
  .validator(photoSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readAlbums, readPhotos } = await import("./photos.server")
    const albums = await readAlbums()

    if (albums.state !== "ready")
      return { albums, page: null, selectedAlbum: null }
    const id = data.album ?? albums.albums.find((album) => !album.missing)?.id

    return {
      albums,
      selectedAlbum: id ?? null,
      page: id === undefined ? null : await readPhotos(id, data.page ?? 0),
    }
  })
