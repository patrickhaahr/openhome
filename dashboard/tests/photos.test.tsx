// @vitest-environment happy-dom
import { act, useState } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { PhotoGallery } from "@/components/photo-gallery"
import {
  chooseView,
  monthSpan,
  photoSearch,
  type Album,
  type AlbumRead,
  type WallRead,
} from "@/lib/photos"

const albumId = "00000000-0000-4000-8000-000000000001"

const otherId = "00000000-0000-4000-8000-000000000004"

const goneId = "00000000-0000-4000-8000-000000000005"

const first = "00000000-0000-4000-8000-000000000002"

const second = "00000000-0000-4000-8000-000000000003"

const third = "00000000-0000-4000-8000-000000000006"

const album: Album = {
  id: albumId,
  name: "Demo places",
  photo_count: 62,
  cover_id: first,
  first_month: "2025-06",
  last_month: "2025-08",
  missing: false,
}

const albums: AlbumRead = {
  state: "ready",
  albums: [
    album,
    {
      ...album,
      id: otherId,
      name: "Other places",
      first_month: "2024-12",
      last_month: "2025-02",
    },
    {
      ...album,
      id: goneId,
      name: "Unavailable album",
      photo_count: 0,
      cover_id: null,
      first_month: null,
      last_month: null,
      missing: true,
    },
  ],
}

const wall: WallRead = {
  state: "ready",
  wall: {
    items: [
      { album: albumId, asset: first },
      { album: albumId, asset: second },
    ],
    page: 0,
    next: 1,
    total: 62,
  },
}

const nextPage: WallRead = {
  state: "ready",
  wall: {
    items: [{ album: albumId, asset: third }],
    page: 1,
    next: null,
    total: 62,
  },
}

type Data = {
  albums: AlbumRead
  view: string | null
  wall: WallRead | null
  photo?: string
}

/** The gallery with its open photo held like the URL holds it. */
async function setup(
  data: Data,
  more: (album: string, page: number) => Promise<WallRead> = async () =>
    nextPage
) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const completion = vi
    .spyOn(HTMLImageElement.prototype, "complete", "get")
    .mockReturnValue(false)

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  const open = vi.fn()
  const shown = vi.fn()
  const load = vi.fn(more)

  function Harness(): React.JSX.Element {
    const [photo, setPhoto] = useState(data.photo)

    return (
      <PhotoGallery
        albums={data.albums}
        view={data.view}
        wall={data.wall}
        photo={photo}
        open={open}
        show={(next) => {
          shown(next)
          setPhoto(next)
        }}
        more={load}
        retry={async () => {}}
      />
    )
  }

  await act(async () => root.render(<Harness />))

  const button = (text: string): HTMLButtonElement => {
    const found = [...document.querySelectorAll("button")].find((item) =>
      item.textContent?.includes(text)
    )

    if (!found) throw new Error(`Missing ${text}`)

    return found
  }

  const preview = (): string | null | undefined =>
    document.querySelector('[role="dialog"] img')?.getAttribute("src")

  return {
    container,
    open,
    shown,
    load,
    button,
    preview,
    async cleanup() {
      await act(async () => root.unmount())
      container.remove()
      completion.mockRestore()
      vi.unstubAllGlobals()
    },
  }
}

test("the albums are cover plates with counts and months, led by all photos", async () => {
  const view = await setup({ albums, view: null, wall: null })

  try {
    const covers = [...view.container.querySelectorAll("img")]
    expect(
      covers.every(
        (img) =>
          img.loading === "lazy" &&
          img.getAttribute("src")?.endsWith("/thumbnail") === true
      )
    ).toBe(true)
    expect(view.button("Demo places").textContent).toContain("62 photos")
    expect(view.button("Demo places").textContent).toContain("Jun – Aug 2025")
    expect(view.button("Other places").textContent).toContain(
      "Dec 2024 – Feb 2025"
    )
    expect(view.button("All photos").textContent).toContain("2 albums")
    expect(view.button("Unavailable album").disabled).toBe(true)
    await act(async () => view.button("Other places").click())
    expect(view.open).toHaveBeenLastCalledWith(otherId)
    await act(async () => view.button("All photos").click())
    expect(view.open).toHaveBeenLastCalledWith("all")
  } finally {
    await view.cleanup()
  }
})

test("one configured album opens directly; a chosen view always wins", () => {
  expect(chooseView([album], undefined)).toBe(albumId)
  expect(chooseView([album, album], undefined)).toBeUndefined()
  expect(chooseView([album, album], "all")).toBe("all")
  expect(photoSearch.parse({ album: "nope", photo: "x" })).toEqual({})
  expect(photoSearch.parse({ album: "all", photo: first })).toEqual({
    album: "all",
    photo: first,
  })
})

test("months read as one month, a span within a year or across years", () => {
  expect(monthSpan("2025-06", "2025-06")).toBe("Jun 2025")
  expect(monthSpan("2025-06", "2025-08")).toBe("Jun – Aug 2025")
  expect(monthSpan("2024-12", "2025-02")).toBe("Dec 2024 – Feb 2025")
  expect(monthSpan(null, null)).toBeNull()
})

test("the wall loads more on request and the viewer steps across pages with one preview", async () => {
  const view = await setup({ albums, view: albumId, wall })

  try {
    expect(view.container.querySelectorAll(".photo-wall img")).toHaveLength(2)
    await act(async () => view.button("All albums").click())
    expect(view.open).toHaveBeenLastCalledWith(undefined)

    const trigger = view.container.querySelector<HTMLButtonElement>(
      '[aria-label="Open photo 2"]'
    )

    if (!trigger) throw new Error("Missing photo trigger")
    await act(async () => trigger.click())
    expect(view.shown).toHaveBeenLastCalledWith(second)
    const dialog = document.querySelector('[role="dialog"]')
    expect(dialog?.textContent).toContain("Photo 2 of 62")
    expect(document.querySelectorAll('[role="dialog"] img')).toHaveLength(1)
    expect(view.preview()).toBe(`/photo-media/${albumId}/${second}/preview`)

    // The last loaded photo's next reads the following page, then shows its first photo.
    await act(async () => view.button("Next →").click())
    expect(view.load).toHaveBeenCalledWith(albumId, 1)
    expect(view.preview()).toBe(`/photo-media/${albumId}/${third}/preview`)
    expect(view.button("Next →").disabled).toBe(true)
    await act(async () => view.button("← Previous").click())
    expect(view.preview()).toBe(`/photo-media/${albumId}/${second}/preview`)
    await act(async () => view.button("Close photo").click())
    expect(view.shown).toHaveBeenLastCalledWith(undefined)
    await vi.waitFor(() =>
      expect(document.querySelector('[role="dialog"]')).toBeNull()
    )
    expect(view.container.querySelectorAll(".photo-wall img")).toHaveLength(3)
    expect(view.container.querySelector(".photo-more button")).toBeNull()
  } finally {
    await view.cleanup()
  }
})

test("a failed page keeps what loaded and offers to try again", async () => {
  const view = await setup({ albums, view: "all", wall }, async () => ({
    state: "unavailable",
    reason: "provider-unavailable",
  }))

  try {
    expect(view.container.textContent).toContain("All photos")
    await act(async () => view.button("Load more").click())
    expect(view.load).toHaveBeenCalledWith("all", 1)
    expect(view.button("Try again").disabled).toBe(false)
    expect(view.container.querySelectorAll(".photo-wall img")).toHaveLength(2)
  } finally {
    await view.cleanup()
  }
})

test("a photo opened from the URL can retry its preview, and stepping clears the failure", async () => {
  const view = await setup({ albums, view: albumId, wall, photo: first })

  try {
    const image = document.querySelector('[role="dialog"] img')

    if (!image) throw new Error("Missing preview")
    await act(async () => image.dispatchEvent(new Event("error")))
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain(
      "Photo unavailable"
    )
    await act(async () => view.button("Retry photo").click())
    expect(view.preview()).toContain("?retry=1")
    await act(async () => view.button("Next →").click())
    expect(view.preview()).toBe(`/photo-media/${albumId}/${second}/preview`)
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
    view: null,
    wall: null,
    expected: "Photo setup needed",
  },
  {
    albums: { state: "unavailable", reason: "provider-unavailable" } as const,
    view: null,
    wall: null,
    expected: "Photo library unavailable",
  },
  {
    albums,
    view: albumId,
    wall: { state: "unavailable", reason: "not-found" } as const,
    expected: "Album unavailable",
  },
  {
    albums,
    view: albumId,
    wall: {
      state: "ready",
      wall: { items: [], page: 0, next: null, total: 0 },
    } satisfies WallRead,
    expected: "No photos here yet",
  },
])(
  "renders $expected honestly without loading photos",
  async ({
    albums: inputAlbums,
    view: inputView,
    wall: inputWall,
    expected,
  }) => {
    const view = await setup({
      albums: inputAlbums,
      view: inputView,
      wall: inputWall,
    })

    try {
      expect(view.container.textContent).toContain(expected)
      expect(view.container.querySelector("img")).toBeNull()
      expect(view.container.textContent).not.toContain("Refresh")
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
      view={albumId}
      wall={wall}
      photo={undefined}
      open={() => {}}
      show={() => {}}
      more={async () => nextPage}
      retry={async () => {}}
    />
  )

  container.innerHTML = renderToString(content)
  document.body.append(container)

  for (const image of container.querySelectorAll("img")) {
    Object.defineProperties(image, {
      complete: { value: true },
      naturalWidth: { value: 400 },
      naturalHeight: { value: 600 },
    })
  }

  const root = hydrateRoot(container, content)

  try {
    await act(async () => {})
    expect(container.textContent).not.toContain("Loading photo…")
    expect(container.querySelectorAll('[data-state="loaded"]')).toHaveLength(2)
    // Portrait thumbnails take a portrait cell once their frame is known.
    expect(
      container
        .querySelector<HTMLElement>(".photo-cell")
        ?.style.getPropertyValue("--ratio")
    ).toBe(String(400 / 600))
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.unstubAllGlobals()
  }
})
