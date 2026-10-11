# Issue tracker: GitHub

Issues and specs for this repo live in
[patrickhaahr/openhome](https://github.com/patrickhaahr/openhome/issues).
The user explicitly selected GitHub issues and PRs for delivery. Do not create
local Markdown implementation tickets under `.scratch/`.

## Conventions

- Publish one GitHub issue per feature spec or implementation ticket.
- Apply the existing `ready-for-agent` label to agent-grabbable specs/tickets.
- Attach implementation tickets as native sub-issues of their feature spec.
- Set genuine prerequisites as native GitHub blocked-by relationships and
  publish blockers before dependents. Use issue IDs, not issue numbers, in the
  relationship API payloads.
- Preserve parent issue bodies and states when slicing; adding child links does
  not authorize editing or closing the parent.
- Keep optional backlog separate from required release gates.
- Public issue bodies, comments, fixtures and PR evidence contain no household
  PII, private coordinates, hostnames, credentials or session material.

## When a skill says "publish to the issue tracker"

Create GitHub issues through the configured authenticated tracker or `gh` inside
devenv. Use structured bodies or `--body-file` so Markdown/newlines survive.
The [dashboard release tracker](https://github.com/patrickhaahr/openhome/issues/69)
contains required feature parents; optional backlog specs are linked separately.

## When a skill says "fetch the relevant ticket"

Fetch the full GitHub issue body and comments, plus its parent/children and
blocked-by relationships when relevant. Use the current tracker state rather
than treating a local index as authoritative.
