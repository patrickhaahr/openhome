import { readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { audit } from "./css-classes.ts"

/**
 * Page stylesheets that still define plain classes while the Tailwind migration (#305) is under way.
 * Delete an entry in the same change that deletes its file; the check fails on a stale entry.
 */
const allowlist = [
  "src/backlight.css",
  "src/components/idle-milestone.css",
  "src/controls.css",
  "src/home.css",
  "src/idle.css",
  "src/photos.css",
  "src/travel.css",
]

const root = new URL("..", import.meta.url).pathname

const sources = new Map(
  readdirSync(join(root, "src"), { recursive: true, encoding: "utf8" })
    .filter((path) => path.endsWith(".css"))
    .sort()
    .map((path) => [
      `src/${path}`,
      readFileSync(join(root, "src", path), "utf8"),
    ])
)

const problems = audit(sources, allowlist)

for (const problem of problems) console.error(problem)

if (problems.length > 0) process.exit(1)
