// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { features } from "@/lib/features"
import { navLabelsFromCookies, navLayoutFromCookies } from "@/lib/nav-layout"
import { Route as ConnectionRoute } from "@/routes/connection"
import { routeTree } from "@/routeTree.gen"

const originalLoader = ConnectionRoute.options.loader

let root: Root

const labels = features.map((feature) => feature.label)

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  ConnectionRoute.options.loader = () =>
    Promise.resolve({
      state: "connected" as const,
      checkedAt: "2026-10-17T10:00:00.000Z",
    })
  document.cookie = "openhome-nav=; Path=/; Max-Age=0"
  document.cookie = "openhome-nav-labels=; Path=/; Max-Age=0"
  root = createRoot(document)
})

afterEach(() => {
  act(() => root.unmount())

  if (originalLoader !== undefined)
    ConnectionRoute.options.loader = originalLoader

  document.cookie = "openhome-nav=; Path=/; Max-Age=0"
  document.cookie = "openhome-nav-labels=; Path=/; Max-Age=0"
  vi.unstubAllGlobals()
})

async function open(): Promise<void> {
  const history = createMemoryHistory({ initialEntries: ["/connection"] })
  const router = createRouter({ routeTree, history, isServer: false })

  await router.load()
  await act(async () => {
    root.render(createElement(RouterProvider, { router }))
  })
}

/** Each link's accessible name: its text, with every glyph hidden from assistive technology. */
function names(selector: string): Array<string | null> {
  const links = [...document.querySelectorAll<HTMLAnchorElement>(selector)]

  for (const link of links)
    for (const glyph of link.querySelectorAll("svg"))
      expect(glyph.getAttribute("aria-hidden")).toBe("true")

  return links.map((link) => link.textContent)
}

function press(group: string, option: string): void {
  const button = [
    ...document.querySelectorAll<HTMLButtonElement>(
      `[role="group"][aria-label="${group}"] button`
    ),
  ].find((candidate) => candidate.textContent === option)

  if (button === undefined) throw new Error(`Expected ${option} in ${group}`)

  act(() => button.click())
}

const shell = (): HTMLElement => document.querySelector<HTMLElement>(".shell")!

test("icon tabs keep every word as the accessible name, in the top bar and the sidebar", async () => {
  document.cookie = "openhome-nav-labels=icons; Path=/"
  await open()

  expect(shell().dataset.labels).toBe("icons")
  expect(names(".topnav a")).toEqual(labels)

  // A fresh page load: the shell reads its choices once, as the server does.
  act(() => root.unmount())
  root = createRoot(document)
  document.cookie = "openhome-nav=side; Path=/"
  await open()

  expect(shell().dataset.nav).toBe("side")
  expect(names(".rail-nav a")).toEqual(labels)
})

test("the Connection page switches the layout and the top bar's labels and remembers both", async () => {
  await open()

  expect(shell().dataset.nav).toBe("top")
  expect(shell().dataset.labels).toBe("words")

  press("Top bar tabs", "Both")
  expect(shell().dataset.labels).toBe("both")
  expect(navLabelsFromCookies(document.cookie)).toBe("both")
  expect(names(".topnav a")).toEqual(labels)

  press("Navigation", "Sidebar")
  expect(shell().dataset.nav).toBe("side")
  expect(document.querySelector(".topbar")).toBeNull()
  expect(navLayoutFromCookies(document.cookie)).toBe("side")
})
