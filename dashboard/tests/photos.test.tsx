// @vitest-environment happy-dom
import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { PhotoGallery } from "@/components/photo-gallery"
import type { AlbumRead, PhotoRead } from "@/lib/photos"

const albumId = "00000000-0000-4000-8000-000000000001"

const first = "00000000-0000-4000-8000-000000000002"

const second = "00000000-0000-4000-8000-000000000003"

const album = {
  id: albumId,
  name: "Demo places",
  photo_count: 62,
  cover_id: first,
  missing: false,
}

const albums: AlbumRead = {
  state: "ready",
  albums: [album, { ...album, id: second, name: "Other places" }],
}

const page: PhotoRead = {
  state: "ready",
  photos: { album, assets: [first, second], page: 0, next_page: 1 },
}

async function setup(data: { albums: AlbumRead; page: PhotoRead | null }) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const completion = vi
    .spyOn(HTMLImageElement.prototype, "complete", "get")
    .mockReturnValue(false)

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  const select = vi.fn()
  const refresh = vi.fn(async () => {})
  await act(async () =>
    root.render(<PhotoGallery {...data} select={select} refresh={refresh} />)
  )

  const button = (text: string): HTMLButtonElement => {
    const found = [...document.querySelectorAll("button")].find((item) =>
      item.textContent?.includes(text)
    )

    if (!found) throw new Error(`Missing ${text}`)

    return found
  }

  return {
    container,
    root,
    select,
    refresh,
    button,
    async cleanup() {
      await act(async () => root.unmount())
      container.remove()
      completion.mockRestore()
      vi.unstubAllGlobals()
    },
  }
}

test("album/page navigation bounds thumbnails and the viewer loads only the selected resized photo", async () => {
  const view = await setup({ albums, page })

  try {
    const thumbnails = [...view.container.querySelectorAll("img")]
    expect(thumbnails).toHaveLength(2)
    expect(
      thumbnails.every(
        (img) =>
          img.loading === "lazy" &&
          img.getAttribute("src")?.endsWith("/thumbnail") === true
      )
    ).toBe(true)
    await act(async () => view.button("Other places").click())
    expect(view.select).toHaveBeenLastCalledWith(second, 0)
    await act(async () => view.button("Next page").click())
    expect(view.select).toHaveBeenLastCalledWith(albumId, 1)

    const trigger = view.container.querySelector<HTMLButtonElement>(
      '[aria-label="Open photo 1"]'
    )

    if (!trigger) throw new Error("Missing photo trigger")
    await act(async () => trigger.click())
    const dialog = document.querySelector('[role="dialog"]')
    expect(dialog?.textContent).toContain("Photo 1 of 62")
    expect(dialog?.querySelector("img")?.getAttribute("src")).toBe(
      `/photo-media/${albumId}/${first}/preview`
    )
    await act(async () => view.button("Next →").click())
    expect(
      document.querySelector('[role="dialog"] img')?.getAttribute("src")
    ).toBe(`/photo-media/${albumId}/${second}/preview`)
    expect(view.button("Next →").disabled).toBe(true)
    await act(async () => view.button("Close photo").click())
    await vi.waitFor(() =>
      expect(document.querySelector('[role="dialog"]')).toBeNull()
    )
    expect(
      view.container.querySelectorAll('img[src$="/preview"]')
    ).toHaveLength(0)
  } finally {
    await view.cleanup()
  }
})

test("missing media can retry and changing photos clears its failed image state", async () => {
  const view = await setup({ albums, page })

  try {
    const trigger = view.container.querySelector<HTMLButtonElement>(
      '[aria-label="Open photo 1"]'
    )

    if (!trigger) throw new Error("Missing photo trigger")
    await act(async () => trigger.click())
    const image = document.querySelector('[role="dialog"] img')

    if (!image) throw new Error("Missing preview")
    await act(async () => image.dispatchEvent(new Event("error")))
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain(
      "Photo unavailable"
    )
    await act(async () => view.button("Retry photo").click())
    expect(
      document.querySelector('[role="dialog"] img')?.getAttribute("src")
    ).toContain("?retry=1")
    await act(async () => view.button("Next →").click())
    expect(
      document.querySelector('[role="dialog"] img')?.getAttribute("src")
    ).toBe(`/photo-media/${albumId}/${second}/preview`)
    expect(
      document.querySelector('[role="dialog"]')?.textContent
    ).not.toContain("Photo unavailable")
  } finally {
    await view.cleanup()
  }
})

test.each([
  {
    albums: { state: "unavailable", reason: "missing-configuration" } as const,
    page: null,
    expected: "Photo setup needed",
  },
  {
    albums: { state: "unavailable", reason: "provider-unavailable" } as const,
    page: null,
    expected: "Photo library unavailable",
  },
  {
    albums,
    page: { state: "unavailable", reason: "not-found" } as const,
    expected: "Album unavailable",
  },
  {
    albums,
    page: {
      state: "ready",
      photos: {
        album: { ...album, photo_count: 0 },
        assets: [],
        page: 0,
        next_page: null,
      },
    } satisfies PhotoRead,
    expected: "No photos here yet",
  },
])(
  "renders $expected honestly without loading photos",
  async ({ albums: inputAlbums, page: inputPage, expected }) => {
    const view = await setup({ albums: inputAlbums, page: inputPage })

    try {
      expect(view.container.textContent).toContain(expected)
      expect(view.container.querySelector("img")).toBeNull()
    } finally {
      await view.cleanup()
    }
  }
)

test("photos loaded before hydration do not remain behind loading placeholders", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const { renderToString } = await import("react-dom/server")
  const { hydrateRoot } = await import("react-dom/client")
  const container = document.createElement("div")

  const content = (
    <PhotoGallery
      albums={albums}
      page={page}
      select={() => {}}
      refresh={async () => {}}
    />
  )

  container.innerHTML = renderToString(content)
  document.body.append(container)

  for (const image of container.querySelectorAll("img")) {
    Object.defineProperties(image, {
      complete: { value: true },
      naturalWidth: { value: 400 },
    })
  }

  const root = hydrateRoot(container, content)

  try {
    await act(async () => {})
    expect(container.textContent).not.toContain("Loading photo…")
    expect(container.querySelectorAll('[data-state="loaded"]')).toHaveLength(2)
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.unstubAllGlobals()
  }
})
