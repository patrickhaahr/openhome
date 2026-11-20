import { readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { audit } from "./css-classes.ts"

const allowlist: string[] = []

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
