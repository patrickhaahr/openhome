import { expect, test } from "vitest"
import {
  dataModeCookie,
  dataModeFromCookies,
  dataModeSwitch,
  parseDataMode,
} from "@/lib/demo/data-mode"

test("anything but the demo value reads as live", () => {
  expect(parseDataMode("demo")).toBe("demo")
  expect(parseDataMode("live")).toBe("live")
  expect(parseDataMode(undefined)).toBe("live")
  expect(parseDataMode("on")).toBe("live")
})

test("the cookie written for a mode reads back as that mode", () => {
  for (const mode of ["live", "demo"] as const)
    expect(
      dataModeFromCookies(
        `openhome-nav=top; ${dataModeCookie(mode).split(";")[0]}`
      )
    ).toBe(mode)
})

test("?demo=on and ?demo=off pick a mode and drop only that parameter", () => {
  expect(
    dataModeSwitch(new URL("https://home.test/workout?view=body&demo=on"))
  ).toEqual({ mode: "demo", location: "/workout?view=body" })
  expect(dataModeSwitch(new URL("https://home.test/?demo=off"))).toEqual({
    mode: "live",
    location: "/",
  })
})

test("other values of the parameter are left alone", () => {
  expect(
    dataModeSwitch(new URL("https://home.test/?demo=maybe"))
  ).toBeUndefined()
  expect(dataModeSwitch(new URL("https://home.test/"))).toBeUndefined()
})
