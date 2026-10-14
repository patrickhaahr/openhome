import assert from "node:assert/strict"

const origin = "http://127.0.0.1:3000"

let response: Response | undefined

for (let attempt = 0; attempt < 30; attempt += 1) {
  try {
    response = await fetch(`${origin}/connection`)
    break
  } catch {
    await new Promise<void>((resolve) => setTimeout(resolve, 500))
  }
}

assert.ok(response !== undefined, "Production server must start")

assert.equal(response.status, 200, "Production server must render Connection")

const html = await response.text()

assert.ok(
  html.includes("unreachable"),
  "Connection must report the offline API"
)

const apiKey = process.env.API_KEY

const apiUrl = process.env.OPENHOME_API_URL

assert.ok(
  apiKey !== undefined && apiKey !== "",
  "Smoke test needs its synthetic key"
)

assert.ok(
  apiUrl !== undefined && apiUrl !== "",
  "Smoke test needs its offline API URL"
)

assert.ok(!html.includes(apiKey), "HTML must not expose the API key")

assert.ok(!html.includes(apiUrl), "HTML must not expose the internal API URL")

const assets: string[] = []

for (const match of html.matchAll(/(?:src|href)="(\/assets\/[^"]+)"/g)) {
  const path = match[1]
  assert.ok(path !== undefined)
  assets.push(path)
}

assert.ok(
  assets.some((path) => path.endsWith(".css")),
  "Page must have styles"
)

for (const path of new Set(assets)) {
  const asset = await fetch(`${origin}${path}`)
  assert.equal(asset.status, 200, `Production asset must exist: ${path}`)
}

console.log(
  "Production image passed: rendered Connection, served CSS/JS assets, and kept credentials private."
)
