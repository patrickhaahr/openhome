import { act } from "react"

export function nameOf(element: Element): string {
  const label = element.getAttribute("aria-label")

  if (label !== null) return label

  const copy = element.cloneNode(true)

  if (!(copy instanceof Element)) return ""

  for (const hidden of copy.querySelectorAll('[aria-hidden="true"]'))
    hidden.remove()

  return (copy.textContent ?? "").replace(/\s+/g, " ").trim()
}

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

export function currencyShown(scope: ParentNode = document): string {
  const trigger = scope.querySelector('[aria-label="Currency"]')

  if (trigger === null) throw new Error("Missing Currency key")

  return nameOf(trigger.querySelector('[data-slot="select-value"]') ?? trigger)
}

export async function chooseCurrency(
  currency: string,
  scope: ParentNode = document
): Promise<void> {
  const trigger = scope.querySelector('[aria-label="Currency"]')

  if (!(trigger instanceof HTMLElement)) throw new Error("Missing Currency key")

  await act(async () => {
    trigger.click()
  })

  const option = Array.from(document.querySelectorAll('[role="option"]')).find(
    (element) => nameOf(element) === currency
  )

  if (!(option instanceof HTMLElement))
    throw new Error(`Missing currency option: ${currency}`)

  await act(async () => {
    option.click()
  })
}

export function slot(
  name: string,
  scope: ParentNode = document
): Element | null {
  return scope.querySelector(`[data-slot="${name}"]`)
}

export function sheet(): HTMLElement {
  const element = document.querySelector<HTMLElement>('[data-slot="sheet"]')

  if (element === null) throw new Error("Expected an open sheet")

  return element
}

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

export async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}
