/** English-locale numbers, shared so every training figure groups and points the same way. */
const oneDecimal = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 1 })

const wholeNumber = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 0 })

/**
 * A number with at most one decimal, such as "85.4".
 *
 * @param value - The number to format.
 * @returns The formatted number, with a real minus sign for negatives.
 */
export function formatDecimal(value: number): string {
  return oneDecimal.format(value).replace("-", "−")
}

/**
 * A whole number with thousands grouping, such as "9,423".
 *
 * @param value - The number to format.
 * @returns The formatted number, with a real minus sign for negatives.
 */
export function formatWhole(value: number): string {
  return wholeNumber.format(value).replace("-", "−")
}

/**
 * A one-decimal deviation that always shows its sign, such as "+0.1" or "−0.2".
 *
 * @param value - The deviation to format.
 * @returns The signed number; a value that rounds to zero is shown as "0.0" without a sign.
 */
export function formatSigned(value: number): string {
  const text = new Intl.NumberFormat("en-GB", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  }).format(Math.abs(value))

  if (Number(text) === 0) return text

  return `${value < 0 ? "−" : "+"}${text}`
}
