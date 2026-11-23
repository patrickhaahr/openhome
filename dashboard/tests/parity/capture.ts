import type { Browser, Locator, Page, Route } from "playwright-core"
import { PNG } from "pngjs"
import {
  viewports,
  type Scenario,
  type Step,
  type Target,
  type Theme,
  type ViewportName,
} from "./scenarios.ts"

export type Job = {
  readonly scenario: Scenario
  readonly viewport: ViewportName
  readonly theme: Theme
}

export type Shot =
  | {
      readonly state: "captured"
      readonly png: Buffer
      readonly problems: readonly string[]
    }
  | {
      readonly state: "failed"
      readonly reason: string
      readonly problems: readonly string[]
    }

const loopback = new Set(["127.0.0.1", "localhost", "[::1]"])

const mapHost = "tiles.openfreemap.org"

const forbiddenTexts = [
  "Some map tiles could not load",
  "Map tiles unavailable",
  "3D buildings are unavailable",
]

const settleTimeout = 20_000

const flatPng = (() => {
  const png = new PNG({ width: 1, height: 1 })
  png.data.set([154, 154, 150, 255])

  return PNG.sync.write(png)
})()

function mapStyle(pathname: string): string {
  return JSON.stringify({
    version: 8,
    sources: {
      openmaptiles: {
        type: "vector",
        tiles: [`https://${mapHost}/parity/{z}/{x}/{y}.pbf`],
        maxzoom: 14,
      },
    },
    layers: [
      {
        id: "ground",
        type: "background",
        paint: {
          "background-color": pathname.endsWith("/dark")
            ? "#1d1f22"
            : "#eceae4",
        },
      },
    ],
  })
}

async function offline(route: Route, blocked: Set<string>): Promise<void> {
  const url = new URL(route.request().url())

  if (loopback.has(url.hostname)) return route.continue()

  blocked.add(url.host)

  if (url.host === mapHost && url.pathname.startsWith("/styles/"))
    return route.fulfill({
      contentType: "application/json",
      body: mapStyle(url.pathname),
    })

  if (url.host === mapHost) return route.fulfill({ status: 204 })

  if (route.request().resourceType() === "image")
    return route.fulfill({ contentType: "image/png", body: flatPng })

  return route.abort("blockedbyclient")
}

function locate(page: Page, target: Target): Locator {
  return "text" in target
    ? page.getByText(target.text).first()
    : page
        .getByRole(target.role, {
          name: target.name,
          exact: target.exact ?? false,
        })
        .first()
}

async function perform(page: Page, step: Step): Promise<void> {
  switch (step.kind) {
    case "click":
      return locate(page, step.target).click()
    case "press":
      return page.keyboard.press(step.key)
    case "hold-drag": {
      const box = await locate(page, step.target).boundingBox()

      if (box === null) throw new Error("drag target has no box")

      const y = box.y + box.height / 2
      await page.mouse.move(box.x + box.width * step.from, y)
      await page.mouse.down()
      await page.mouse.move(box.x + box.width * step.to, y, { steps: 12 })
    }
  }
}

function networkQuiet(page: Page): () => Promise<void> {
  let open = 0
  let changed = Date.now()

  const touch = (delta: number) => () => {
    open += delta
    changed = Date.now()
  }

  page.on("request", touch(1))
  page.on("requestfinished", touch(-1))
  page.on("requestfailed", touch(-1))

  return async () => {
    const deadline = Date.now() + settleTimeout

    while (open > 0 || Date.now() - changed < 300) {
      if (Date.now() > deadline)
        throw new Error(`network never went quiet (${open} open)`)

      await page.waitForTimeout(50)
    }
  }
}

async function frames(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((done) => {
        requestAnimationFrame(() => requestAnimationFrame(() => done()))
      })
  )
}

async function scrollThrough(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const root = document.scrollingElement ?? document.documentElement

    for (const image of document.images) image.loading = "eager"

    const frame = () =>
      new Promise<void>((done) => {
        requestAnimationFrame(() => done())
      })

    for (let y = 0; y < root.scrollHeight; y += window.innerHeight) {
      root.scrollTo(0, y)
      await frame()
    }

    root.scrollTo(0, 0)
    await frame()
  })
}

async function imagesReady(page: Page, whole: boolean): Promise<void> {
  await page.waitForFunction(
    (everywhere) =>
      [...document.images].every((image) => {
        const box = image.getBoundingClientRect()
        const laidOut = image.getClientRects().length > 0

        const inView =
          box.bottom > 0 &&
          box.top < window.innerHeight &&
          box.right > 0 &&
          box.left < window.innerWidth

        return !laidOut || (!everywhere && !inView) || image.complete
      }),
    whole,
    { timeout: settleTimeout }
  )
  await page.evaluate(async () => {
    await Promise.all(
      [...document.images].flatMap((image) =>
        image.complete && image.naturalWidth > 0
          ? [image.decode().catch(() => null)]
          : []
      )
    )
  })
}

async function themeHolds(page: Page, theme: Theme): Promise<string | null> {
  const root = await page.evaluate(() => ({
    dark: document.documentElement.classList.contains("dark"),
    turning: document.documentElement.classList.contains("theme-turning"),
  }))

  if (root.turning) return "theme still turning"

  return root.dark === (theme === "night") ? null : `theme is not ${theme}`
}

async function stableShot(page: Page, fullPage: boolean): Promise<Buffer> {
  const shoot = () => page.screenshot({ fullPage, animations: "disabled" })
  let previous = await shoot()

  for (let attempt = 0; attempt < 4; attempt += 1) {
    await frames(page)
    const next = await shoot()

    if (next.equals(previous)) return next

    previous = next
  }

  throw new Error("screenshot never settled")
}

async function render(
  page: Page,
  origin: string,
  job: Job,
  problems: string[]
): Promise<Shot> {
  page.on("pageerror", (error) => problems.push(`page error: ${error.message}`))
  page.on("response", (response) => {
    if (response.status() >= 500)
      problems.push(`${response.status()} from ${response.url()}`)
  })
  const quiet = networkQuiet(page)
  const whole = (job.scenario.capture ?? "full") === "full"
  await page.goto(origin + job.scenario.path, { timeout: settleTimeout })
  await quiet()
  await frames(page)

  for (const step of job.scenario.steps ?? []) {
    await perform(page, step)
    await quiet()
  }

  await locate(page, job.scenario.expect).waitFor({ timeout: settleTimeout })
  await page.evaluate(async () => {
    await document.fonts.ready
  })

  if (whole) await scrollThrough(page)

  await imagesReady(page, whole)
  await quiet()

  for (const text of forbiddenTexts)
    if ((await page.getByText(text).count()) > 0)
      problems.push(`shows "${text}"`)

  const theme = await themeHolds(page, job.theme)

  if (theme !== null) return { state: "failed", reason: theme, problems }

  await frames(page)

  return { state: "captured", png: await stableShot(page, whole), problems }
}

export async function capture(
  browser: Browser,
  origin: string,
  job: Job,
  now: number,
  blocked: Set<string>
): Promise<Shot> {
  const size = viewports[job.viewport]

  const context = await browser.newContext({
    viewport: { width: size.width, height: size.height },
    deviceScaleFactor: size.scale,
    reducedMotion: job.scenario.motion ?? "reduce",
    locale: "en-GB",
    timezoneId: "Europe/Copenhagen",
  })

  const problems: string[] = []
  let timer: ReturnType<typeof setTimeout> | undefined

  const deadline = new Promise<never>((_, fail) => {
    timer = setTimeout(
      () => fail(new Error("capture took over 2 minutes")),
      120_000
    )
  })

  try {
    await context.clock.setFixedTime(
      job.scenario.clock === "daytime" ? new Date(now).setUTCHours(12) : now
    )
    await context.addInitScript((mode) => {
      localStorage.setItem("openhome-theme", mode)
    }, job.theme)
    await context.route("**/*", (route) => offline(route, blocked))
    const page = await context.newPage()

    return await Promise.race([render(page, origin, job, problems), deadline])
  } catch (error) {
    const reason = error instanceof Error ? error.message.split("\n")[0] : ""

    return { state: "failed", reason: reason ?? "capture failed", problems }
  } finally {
    clearTimeout(timer)
    await context.close()
  }
}
