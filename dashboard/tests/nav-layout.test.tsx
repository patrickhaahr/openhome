// @vitest-environment happy-dom

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import {
  NavLayoutProvider,
  NavLayoutSwitch,
  useLogoTaps,
  useNavLayout,
} from "@/components/nav-layout"
import {
  navLabelsCookie,
  navLabelsFromCookies,
  navLayoutCookie,
  navLayoutFromCookies,
  type NavLayout,
} from "@/lib/nav-layout"

let root: Root

let container: HTMLDivElement

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  vi.setSystemTime(new Date("2026-10-17T12:00:00.000Z"))
  // happy-dom keeps cookies across tests; expire the one under test.
  document.cookie = "openhome-nav=; Path=/; Max-Age=0"
  document.cookie = "openhome-nav-labels=; Path=/; Max-Age=0"
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(() => {
  act(() => root.unmount())
  container.remove()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

/** A stand-in logo: the shell's logos are router links with this same click handler. */
function Logo({ clicks }: { clicks: Array<boolean> }): React.JSX.Element {
  const { layout } = useNavLayout()
  const onClick = useLogoTaps()

  return (
    <a
      href="/"
      data-layout={layout}
      onClick={(event) => {
        onClick(event)
        clicks.push(event.defaultPrevented)
        // The test stays on this page; the router would go Home here.
        event.preventDefault()
      }}
    >
      openhome
    </a>
  )
}

function render(initial: NavLayout, clicks: Array<boolean> = []): void {
  act(() =>
    root.render(
      <NavLayoutProvider initial={{ layout: initial, labels: "words" }}>
        <Logo clicks={clicks} />
        <NavLayoutSwitch />
      </NavLayoutProvider>
    )
  )
}

const logo = (): HTMLAnchorElement => container.querySelector("a")!

const click = (element: HTMLElement): void => {
  act(() => element.click())
}

test("reads the layout from a cookie header and falls back to the top bar", () => {
  expect(navLayoutFromCookies("a=1; openhome-nav=side; b=2")).toBe("side")
  expect(navLayoutFromCookies("openhome-nav=top")).toBe("top")
  expect(navLayoutFromCookies("openhome-nav=sideways")).toBe("top")
  expect(navLayoutFromCookies("other-openhome-nav=side")).toBe("top")
  expect(navLayoutFromCookies("")).toBe("top")
})

test("remembers the layout for a year on every path", () => {
  expect(navLayoutCookie("side")).toBe(
    "openhome-nav=side; Path=/; Max-Age=31536000; SameSite=Lax"
  )
  document.cookie = navLayoutCookie("side")
  expect(navLayoutFromCookies(document.cookie)).toBe("side")
})

test("reads the top bar's labels from a cookie header and falls back to words", () => {
  expect(
    navLabelsFromCookies("openhome-nav=side; openhome-nav-labels=icons")
  ).toBe("icons")
  expect(navLabelsFromCookies("openhome-nav-labels=both")).toBe("both")
  expect(navLabelsFromCookies("openhome-nav-labels=glyphs")).toBe("words")
  expect(navLabelsFromCookies("openhome-nav=side")).toBe("words")
})

test("remembers the top bar's labels for a year on every path", () => {
  expect(navLabelsCookie("icons")).toBe(
    "openhome-nav-labels=icons; Path=/; Max-Age=31536000; SameSite=Lax"
  )
  document.cookie = navLabelsCookie("both")
  expect(navLabelsFromCookies(document.cookie)).toBe("both")
  expect(navLayoutFromCookies(document.cookie)).toBe("top")
})

test("a single click goes Home without switching; a second within 350 ms switches and stays", () => {
  const clicks: Array<boolean> = []
  render("top", clicks)

  click(logo())
  expect(logo().dataset.layout).toBe("top")
  expect(clicks).toEqual([false])

  vi.advanceTimersByTime(300)
  click(logo())
  expect(logo().dataset.layout).toBe("side")
  expect(clicks).toEqual([false, true])
  expect(navLayoutFromCookies(document.cookie)).toBe("side")

  // The pair is spent: the next click navigates again, and another pair switches back.
  vi.advanceTimersByTime(200)
  click(logo())
  expect(logo().dataset.layout).toBe("side")
  vi.advanceTimersByTime(100)
  click(logo())
  expect(logo().dataset.layout).toBe("top")
  expect(navLayoutFromCookies(document.cookie)).toBe("top")
})

test("two clicks further apart than the window are two trips Home", () => {
  const clicks: Array<boolean> = []
  render("side", clicks)

  click(logo())
  vi.advanceTimersByTime(400)
  click(logo())

  expect(logo().dataset.layout).toBe("side")
  expect(clicks).toEqual([false, false])
})

test("a click the browser numbers as the second of a double click switches on its own", () => {
  render("top")

  act(() => {
    logo().dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true, detail: 2 })
    )
  })

  expect(logo().dataset.layout).toBe("side")
})

test("the plain switch chooses the layout and remembers it", () => {
  render("top")

  const sidebar = [...container.querySelectorAll("button")].find(
    (button) => button.textContent === "Sidebar"
  )!

  expect(sidebar.getAttribute("aria-pressed")).toBe("false")
  click(sidebar)

  expect(sidebar.getAttribute("aria-pressed")).toBe("true")
  expect(logo().dataset.layout).toBe("side")
  expect(navLayoutFromCookies(document.cookie)).toBe("side")
})
