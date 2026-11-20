import { createServer } from "node:http"
import { expect, test } from "vitest"
import { z } from "zod"
import {
  apiKey,
  axumUrl,
  dashboard,
  listen,
  page,
} from "./support/connection-server"

test("renders shared Wishlists through Start SSR and authenticated Axum without exposing credentials", async () => {
  const ownerResponse = await fetch(`${axumUrl}/api/household-users`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ name: "Member A" }),
  })

  const owner = z.object({ id: z.uuid() }).parse(await ownerResponse.json())

  const listResponse = await fetch(`${axumUrl}/api/wishlists`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ owner_id: owner.id, name: "Outdoors" }),
  })

  const list = z.object({ id: z.uuid() }).parse(await listResponse.json())

  await fetch(`${axumUrl}/api/wishlist/items`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      owner_id: owner.id,
      list_id: list.id,
      title: "A warm jacket",
      request_id: "dashboard-ssr-wish",
    }),
  })

  const url = await dashboard({ url: axumUrl, key: apiKey })

  const html = await page(url, "/lists")

  expect(html).toContain("Member A")
  expect(html).toContain("Outdoors")
  expect(html).toContain("A warm jacket")
  expect(html).toContain("Add a wish")
  expect(html).not.toContain("Not available yet")
})

test("gallery SSR and media HTTP routes keep private configuration and upstream headers server-side", async () => {
  const album = "00000000-0000-4000-8000-000000000001"
  const asset = "00000000-0000-4000-8000-000000000002"

  const summary = {
    id: album,
    name: "Demo places",
    photo_count: 1,
    cover_id: asset,
    first_month: "2025-06",
    last_month: "2025-06",
    missing: false,
  }

  const requests: string[] = []
  let mediaStatus = 200

  const upstream = createServer((request, response) => {
    expect(request.headers.authorization).toBe(`Bearer ${apiKey}`)
    requests.push(request.url ?? "")

    if (request.url?.includes("/assets/") === true) {
      response.writeHead(mediaStatus, {
        "Content-Type": "image/webp",
        "X-Provider-Key": "synthetic-private-key",
        Location: "https://private.invalid/image",
      })
      response.end(
        mediaStatus === 200 ? Buffer.from([1, 2, 3]) : "synthetic-private-key"
      )

      return
    }

    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(
      JSON.stringify(
        request.url === "/api/photos/albums"
          ? {
              state: "ready",
              albums: [summary],
              credential: "synthetic-private-key",
            }
          : {
              state: "ready",
              photos: {
                album: summary,
                assets: [asset],
                page: 0,
                next_page: null,
              },
              exif: "synthetic-private-key",
            }
      )
    )
  })

  const url = await dashboard({ url: await listen(upstream), key: apiKey })
  const html = await page(url, "/photos")
  expect(html).toContain("Demo places")
  expect(html).toContain("Jun 2025")
  expect(html).toContain(`/photo-media/${album}/${asset}/thumbnail`)
  expect(html).not.toContain("synthetic-private-key")
  expect(html).not.toContain("private.invalid")
  const image = await fetch(`${url}/photo-media/${album}/${asset}/thumbnail`)
  expect(image.status).toBe(200)
  expect(image.headers.get("cache-control")).toBe("no-store")
  expect(image.headers.get("content-type")).toBe("image/webp")
  expect(image.headers.get("x-provider-key")).toBeNull()
  expect(image.headers.get("location")).toBeNull()
  expect([...new Uint8Array(await image.arrayBuffer())]).toEqual([1, 2, 3])
  const count = requests.length
  expect(
    (await fetch(`${url}/photo-media/${album}/${asset}/original`)).status
  ).toBe(404)
  expect(
    (await fetch(`${url}/photo-media/not-an-id/${asset}/thumbnail`)).status
  ).toBe(404)
  expect(requests).toHaveLength(count)
  mediaStatus = 404
  const missing = await fetch(`${url}/photo-media/${album}/${asset}/preview`)
  expect(missing.status).toBe(404)
  expect(await missing.text()).toBe("")
})

test("wish pictures are relayed as bounded image bytes without upstream headers", async () => {
  const owner = "00000000-0000-4000-8000-000000000011"
  const list = "00000000-0000-4000-8000-000000000012"
  const item = "00000000-0000-4000-8000-000000000013"
  const requests: string[] = []
  let status = 200
  let type = "image/png"

  const upstream = createServer((request, response) => {
    expect(request.headers.authorization).toBe(`Bearer ${apiKey}`)
    requests.push(request.url ?? "")
    response.writeHead(status, {
      "Content-Type": type,
      "X-Provider-Key": "synthetic-private-key",
      Location: "https://private.invalid/image",
    })
    response.end(
      status === 200 ? Buffer.from([1, 2, 3]) : "synthetic-private-key"
    )
  })

  const url = await dashboard({ url: await listen(upstream), key: apiKey })
  const path = `/wish-image/${owner}/${list}/${item}?v=1`
  const image = await fetch(`${url}${path}`)

  expect(image.status).toBe(200)
  expect(requests).toEqual([
    `/api/household-users/${owner}/wishlists/${list}/items/${item}/image`,
  ])
  expect(image.headers.get("content-type")).toBe("image/png")
  expect(image.headers.get("cache-control")).toBe("private, max-age=3600")
  expect(image.headers.get("x-content-type-options")).toBe("nosniff")
  expect(image.headers.get("content-security-policy")).toContain("sandbox")
  expect(image.headers.get("x-provider-key")).toBeNull()
  expect(image.headers.get("location")).toBeNull()
  expect([...new Uint8Array(await image.arrayBuffer())]).toEqual([1, 2, 3])

  expect(
    (await fetch(`${url}/wish-image/not-an-id/${list}/${item}`)).status
  ).toBe(404)
  expect(requests).toHaveLength(1)

  type = "text/html"
  const shop = await fetch(`${url}${path}`)
  expect(shop.status).toBe(503)
  expect(await shop.text()).toBe("")

  status = 404
  type = "application/json"
  const missing = await fetch(`${url}${path}`)
  expect(missing.status).toBe(404)
  expect(missing.headers.get("cache-control")).toBe("no-store")
  expect(await missing.text()).toBe("")
})
