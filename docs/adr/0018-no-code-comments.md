# Code carries no comments

Comments rot. They repeat the code, drift from it, or excuse a shortcut that
then gets copied. So source files carry no comments. The *why* goes into
names, types and tests. Decisions that are hard to reverse go into an ADR, and
known hacks go into a GitHub issue.

`just comment-check` (part of `just check`) enforces this with a ratchet.
`scripts/comment-baseline.json` records how many comments each file may still
hold. A file fails when it gains comments, and the check also fails when a file
drops below its baseline until `just comment-baseline` locks in the lower
count. New files start at zero. The
[delete-comment skill](../../.agents/skills/delete-comment/SKILL.md) is the
repeatable process for shrinking the baseline.

## Exemptions

- Tool directives (`eslint-disable`, `@ts-expect-error`, `noqa`, shebangs, and
  similar). They are configuration, not prose.
- `SAFETY:` comments. The dashboard's
  `require-safety-comment-for-type-assertion` rule requires one on every
  non-const type assertion, and Rust convention requires one on every `unsafe`
  block. Each one marks an escape hatch in the type system, which is itself the
  thing to remove.
- Vendored code under `vendor/` and the generated `dashboard/.intent/` hooks.

These are not exempt, but stay in the baseline for the reasons given:

- Rust doc comments that are runtime data. On MCP argument structs (schemars)
  they become the JSON Schema descriptions Hermes reads. On the CLI (clap)
  they become `--help` text. Deleting them changes behaviour. Moving them into
  `#[schemars(description = ...)]` and `#[arg(help = ...)]` attributes removes
  them from the baseline.
- Licence notices on adapted third-party code. MIT requires them.
- Applied SQL migrations. sqlx checksums each migration file, so editing a
  comment in one that has already run makes `sqlx migrate` reject it in
  production. New migrations carry no comments.

## Considered Options

- Native lint rules: neither clippy nor oxlint can ban comments. A single
  language-agnostic script covers Rust, TypeScript, Kotlin, SQL, Python, shell,
  QML, CSS and C++ the same way.
- A hard ban with no baseline: it would block every change until about 7,000
  comments were triaged in one go.
