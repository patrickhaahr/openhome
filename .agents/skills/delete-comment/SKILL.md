---
name: delete-comment
description: Classify every code comment under a path, delete it, and move what it knew into an ADR or a GitHub issue, shrinking the comment ratchet.
disable-model-invocation: true
---

# Delete comments

Source files carry no comments ([ADR 0018](../../../docs/adr/0018-no-code-comments.md)).
`scripts/comment-baseline.json` is the ratchet: it records how many comments
each file may still hold. One run takes one **scope** (the paths given, or
else the top-level area with the most comments in the baseline) and drives its
count to the floor.

## 1. Inventory

Run `just comment-list <scope>`. It prints one JSON line per comment with
`path`, `line`, `end_line`, `kind` (`line`, `block`, `doc`), `own_lines`
(false means a trailing comment) and `text`. Consecutive `//` or `#` lines are
separate entries, so group adjacent own-line entries into one **block** before
judging them.

Done when every block in scope is in your working list.

## 2. Classify every block

Read each block together with the code it sits on, then give it exactly one
label:

- **Tautology**: restates what the names, types or control flow already say.
  This includes section banners, `// namespace` closers, and docs that
  paraphrase a function's signature.
- **Stale**: describes code, history or behaviour that is gone, or repeats a
  source of truth that lives elsewhere (an ADR, `docs/`, a skill, a rule's
  `meta.docs`).
- **Excuse**: makes up for a shortcut. Examples: a tuple whose fields are
  named only in a comment, a blanket lint `allow` with a justification, magic
  numbers derived in prose, a lifecycle spread across loose variables and
  explained in a comment, or rules kept "in step with" another copy.
- **Decision record**: a non-obvious constraint or choice the code cannot
  show. Examples: a platform quirk, a security invariant, a protocol limit, or
  the reason an alternative was rejected.

Leave these in place: they are outside the four labels.

- Rust doc comments that are runtime data: on `schemars::JsonSchema` types in
  `mcp/src` they become the tool schema descriptions Hermes reads, and on clap
  types in `cli/src` they become `--help` text. To shrink them, move the text
  into `#[schemars(description = "...")]` or `#[arg(help = "...")]` /
  `#[command(about = "...")]`, then check that the output is unchanged
  (`mcp.sh tools/list`, `--help`).
- Licence notices on adapted third-party code.
- Every file under `api/*migrations/`. sqlx checksums applied migrations, so
  editing one breaks `sqlx migrate` on the Pi.

The scanner already skips directives (`eslint-disable`, `@ts-expect-error`,
`noqa`, `@vitest-environment`, `noinspection` and similar), `SAFETY:` notes
with their continuation lines, `vendor/`, `dashboard/.intent/` and
`routeTree.gen.ts`.

Done when every block has a label, a reason in a few words, and its
destination.

## 3. Move what each block knew

- **Decision record**: write it into `docs/adr/`. Group the records by theme
  and add to an existing ADR when one covers that theme (0019 fixtures, 0020
  MCP transport, 0021 releases, 0022 TV runtime). Otherwise start the next
  number, using the format in the existing ADRs. Phrase each record as a
  constraint plus its reason.
- **Excuse**: file one GitHub issue per hack, following
  [the issue tracker conventions](../../../docs/agents/issue-tracker.md):
  the location, what the comment excused, and the fix that would make it
  unnecessary. Label it `ready-for-agent`, and group sites that share one fix
  into a single issue.

Public text (ADRs, issues) carries no household PII, private addresses,
hostnames or credentials.

Done when every decision record is in an ADR and every excuse is in an issue.

## 4. Delete

Pass every entry's `path:line` (each line of a block) to
`python3 scripts/comments.py strip <path:line>...`. It removes own-line
comments together with the blank line they leave behind, and cuts trailing
comments off their line; a JSX `{/* ... */}` goes with its braces. Then run the formatter for each area touched.

Done when `just comment-list <scope>` shows only the entries you labelled to
keep.

## 5. Lock in the ratchet

Run `just comment-baseline`, then every touched area's `just <area>-check`
plus `just comment-check`. The PR body lists the counts per label, the ADRs
touched, the issues filed, and the lines removed.

Done when all the gates pass and the baseline diff only shrinks.
