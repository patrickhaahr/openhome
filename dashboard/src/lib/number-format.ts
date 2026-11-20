const oneDecimal = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 1 })

const wholeNumber = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 0 })

export function formatDecimal(value: number): string {
  return oneDecimal.format(value).replace("-", "−")
}

export function formatWhole(value: number): string {
  return wholeNumber.format(value).replace("-", "−")
}

export function formatSigned(value: number): string {
  const text = new Intl.NumberFormat("en-GB", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  }).format(Math.abs(value))

  if (Number(text) === 0) return text

  return `${value < 0 ? "−" : "+"}${text}`
}
