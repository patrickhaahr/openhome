import { act } from "react"

/** The accessible text of an element: aria-label, else its text without decorative parts. */
export function nameOf(element: Element): string {
  const label = element.getAttribute("aria-label")

  if (label !== null) return label

  const copy = element.cloneNode(true)

  if (!(copy instanceof Element)) return ""

  for (const hidden of copy.querySelectorAll('[aria-hidden="true"]'))
    hidden.remove()

  return (copy.textContent ?? "").replace(/\s+/g, " ").trim()
}

/** The first button named exactly `name` (or matching a pattern) within `scope`. */
export function button(
  name: string | RegExp,
  scope: ParentNode = document
): HTMLButtonElement {
  const match = Array.from(scope.querySelectorAll("button")).find((element) =>
    name instanceof RegExp
      ? name.test(nameOf(element))
      : nameOf(element) === name
  )

  if (match === undefined) throw new Error(`Missing button: ${String(name)}`)

  return match
}

/** The open detail plate; sheets render in a portal beside the page. */
export function sheet(): HTMLElement {
  const element = document.querySelector<HTMLElement>(".sheet")

  if (element === null) throw new Error("Expected an open sheet")

  return element
}

/** A labelled input or textarea within `scope`. */
export function field(
  label: string,
  scope: ParentNode = document
): HTMLInputElement | HTMLTextAreaElement {
  const matched = Array.from(scope.querySelectorAll("label")).find(
    (element) => (element.textContent ?? "").trim() === label
  )

  const input = document.getElementById(matched?.htmlFor ?? "")

  if (
    !(input instanceof HTMLInputElement) &&
    !(input instanceof HTMLTextAreaElement)
  )
    throw new Error(`Missing field: ${label}`)

  return input
}

/** Type as a person would: through the native setter, so React sees the change. */
export async function type(
  input: HTMLInputElement | HTMLTextAreaElement,
  value: string
): Promise<void> {
  await act(async () => {
    const prototype =
      input instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype

    Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(input, value)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
}

/** Settle timers and promises queued by a click, such as closing a sheet after a save. */
export async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}
