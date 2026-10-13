# Dashboard Profile and Body Weight

Issue [#122](https://github.com/patrickhaahr/openhome/issues/122), beneath
[#81](https://github.com/patrickhaahr/openhome/issues/81).

The Workout view now opens Profile & weight at `/workout?view=body`.
It uses the existing authenticated `/api/profile` and `/api/body_weight`
contracts, with the Axum credential kept on the dashboard server. Fitness
ownership remains shared. Unchanged Profile fields are omitted; clearing an
edited field sends explicit null. Body Weight is append-only with one record
per date. History reads are chronological, bounded to 500 entries, and filtered
by optional inclusive `from`/`to` dates in the URL. A full page shows a limit
notice so it cannot be mistaken for complete history.

## Public evidence

All captures use the isolated `dashboard-health-fixture-run` API and synthetic
values, with no household data. Before captures show the target branch's
Workout view at `b1cdeaf`; after captures show the production build preview.

| View | Before | After |
| --- | --- | --- |
| Landscape, 1024 × 768 | [Workout](evidence/body-profile-before-landscape.png) | [Profile & weight](evidence/body-profile-after-landscape.png) |
| Portrait, 768 × 1024 | [Workout](evidence/body-profile-before-portrait.png) | [Profile & weight](evidence/body-profile-after-portrait.png) |

[Duplicate date](evidence/body-profile-duplicate-date.png) shows a rejected
71.8 kg draft alongside the unchanged 72.5 kg record.

## Verification

- `devenv shell -- just dashboard-check`: format, lint, typecheck, 121 tests,
  production build.
- Rendered forms against real authenticated Axum routes and migrated SQLite:
  Profile creation, exact stored values, concurrent untouched fields, explicit
  null clearing, body-weight persistence, duplicate-date conflicts, filtering,
  invalid drafts and rejected-credential recovery. Review regressions also
  verify that a failed read after a confirmed write preserves an active Profile
  draft, enables Refresh and keeps the persistence receipt visible.
- Production-browser Profile and Body Weight writes, duplicate-date recovery,
  landscape/portrait screenshots and no horizontal overflow. Inputs and
  buttons measure 48 px high; view links measure 56 px high.
- Automated WCAG A/AA audit: zero violations; one incomplete check on the
  pre-existing sidebar brand's accessible label.

These are desktop-browser layout checks. Physical iPad Safari verification
remains an integrated release gate under #69.
