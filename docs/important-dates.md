# Important Dates

The household's shared list of dates it counts from or towards
([#194](https://github.com/patrickhaahr/openhome/issues/194), part of
[#193](https://github.com/patrickhaahr/openhome/issues/193)). Calendars stay the
place for reminders; OpenHome owns counting from a date.

An Important Date has a `title` (1–200 characters), a `date` (`YYYY-MM-DD`), a
`kind` and a `show_when_idle` switch. It is never owned. Instead it tags 0..n
Household Users through `person_ids`: both members for a shared anniversary, one
for a birthday, none for a friend's birthday. Removing a Household User removes
only their tags.

- `since` counts up from the date: elapsed years, months and days, plus total days.
- `yearly` counts down to the date's next anniversary.

## Day arithmetic

Axum computes every figure for one Europe/Copenhagen calendar day, which is
today unless `?on=YYYY-MM-DD` is given. Clock time and daylight saving never
enter: a figure changes only at Copenhagen midnight, and a 23- or 25-hour day is
one day.

- **Elapsed** counts whole months from the date itself, clamped to the end of
  shorter months, then the remaining days. 31 January to 28 February is one
  month exactly, and 31 January to 1 March is one month and one day.
- **29 February** has no anniversary in a common year. It falls back to
  28 February, both for the next occurrence and for whole years: 29 Feb 2024 to
  28 Feb 2025 is one year.
- **Next** is the first anniversary on or after the day. `days_until` is 0 on
  the day itself, and `years` says which anniversary it is. A date that is still
  ahead is its own next occurrence (`years` 0) and has no `elapsed` figure (null).

Both figures are returned for both kinds. A view leads with `elapsed` for since
dates and `next` for yearly dates. The idle screen (#195) and milestones (#196)
use the same figures for any `on` day.

## Milestones

Axum adds `milestone` to every date, including single-date and mutation responses:
`null`, `{ "kind": "anniversary", "years": 2 }`,
`{ "kind": "day_count", "days": 1000 }` or
`{ "kind": "yearly", "years": 34 }`. Existing fields and list order stay unchanged.

- Since dates celebrate every anniversary after the original day. The original
  day (zero years) is not an anniversary milestone.
- Since dates also celebrate exactly 100, 500, 1,000 days and every 1,000 days
  thereafter. An anniversary wins if both rules fall on the same day.
- Yearly dates celebrate the occurrence itself, including the original day.
  `years` is the number of years from the stored date, zero on the original day.
  The title stays literal: no birthday semantics or unknown birth year is inferred.
- Milestones use the same Copenhagen calendar day, DST and 29 February fallback
  as the other figures. No milestone appears before its date or on adjacent days.

The Dates tab leads with each milestone even for dates hidden from idle. Idle
reads should use `show_when_idle=true`; `IdleMilestone` additionally checks this
flag before rendering. Its lead priority is anniversary, day count, then yearly;
ties keep the API's existing order (oldest since date, then title and ID; yearly
dates by next occurrence, then title and ID).

`dashboard/src/components/idle-milestone.tsx` exports
`<IdleMilestone dates={data.dates} />` as a passive lead slot. The parent owns
loading and Copenhagen-midnight refresh; pass the current successful read, or
an empty array when unavailable. Wire the shared idle read with
`<IdleDates lead={(dates) => <IdleMilestone dates={dates} />} />`.
It renders one milestone or no markup, never
fetches, and has no animation (including under reduced motion). A yearly date
keeps its title as the lead and shows years since the stored date underneath
when nonzero; this conveys age for a stored birthday without guessing its kind.


## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. Dashboard holds the
credential on the TanStack Start server. MCP forwards one authenticated Axum
request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `important_dates` | `GET /api/important-dates` | Optional `on` (`YYYY-MM-DD`) and `show_when_idle` filter |
| `important_date` | `GET /api/important-dates/{id}` | Date ID; optional `on` on the API |
| `add_important_date` | `POST /api/important-dates` | `title`, `date`, `kind`, `person_ids` (required, may be `[]`), optional `show_when_idle` (default false), `request_id` |
| `update_important_date` | `PATCH /api/important-dates/{id}` | Any of `title`, `date`, `kind`, `person_ids`, `show_when_idle` |
| `delete_important_date` | `DELETE /api/important-dates/{id}` | Date ID |

The list answers:

```json
{
  "on": "2026-05-26",
  "dates": [
    {
      "id": "…",
      "title": "Together",
      "date": "2024-02-14",
      "kind": "since",
      "person_ids": ["…", "…"],
      "show_when_idle": true,
      "elapsed": { "years": 2, "months": 3, "days": 12, "total_days": 832 },
      "next": { "date": "2027-02-14", "days_until": 264, "years": 3 },
      "milestone": null
    }
  ]
}
```

Since dates come first (oldest first), then yearly dates by next occurrence.
`person_ids` are ordered by name. A single date, an addition
(`{ "important_date": …, "replayed": false }`) and an edit return the same date
shape, with figures for today.

Tags are never inferred: `person_ids` is required on addition, each ID must be a
configured Household User (404 otherwise) and listed once (400). `PATCH`
changes only the supplied fields, and none can be null; `person_ids` replaces
every tag. Additions follow the Wishlist retry convention: reuse the same
`request_id` and arguments after an uncertain response to get the original date
with `replayed: true` (200). Changed arguments conflict (409), and a deleted date
cannot be recreated by an old `request_id`. A rejected addition consumes no ID.
Deletion returns 204, mapped to MCP `{ "deleted": true }`.

Migration 0013 stores dates, their tags (cascading from the date and the
Household User) and immutable addition receipts.

## Dashboard

`/lists?tab=dates` is the third Lists tab, beside Wish and Watch. It shares the
person filter: a person shows their tagged dates plus untagged ones. Each date is
a plate with its title, a kind glyph, the live figure ("7 years · 8 months ·
3 days", "in 18 days", "Tomorrow", "Today" with a sun disc), the date and its
people. Tapping a plate opens a detail sheet with Edit, the Show When Idle switch
and Delete. "Add a date" opens the same fields. The view re-reads when the
Copenhagen day turns.

## Synthetic preview

`api/examples/dashboard_preview/seed/dates.json` seeds invented dates for the
synthetic people: a `since` "Together" (14 Feb) tagging two people, a "First date"
(3 Jun, yearly), birthdays, a leap-day birthday that tags no one, and "Moved in"
seeded exactly 1,000 days ago, so today's milestone is always visible.

## Idle display

Only dates explicitly switched to **Show when idle** appear on the room display.
The dashboard requests `GET /api/important-dates?show_when_idle=true` when idle
begins and again at Europe/Copenhagen midnight, including across daylight-saving
changes. Axum remains the owner of elapsed and countdown arithmetic.

Each since date shows its title, years/months/days and smaller total days on the
clock's half-clear night-ground plate. One yearly line rotates every 12 seconds,
nearest first, saying "today" or "tomorrow" where appropriate. Lines swap without
travel for every motion preference. Empty or failed reads quietly leave the clock
and weather. `IdleDates` accepts optional `lead` content above the dates, either a node or a
render function receiving the filtered dates. For example,
`<IdleDates lead={(dates) => <IdleMilestone dates={dates} />} />` shares the same
fetch and midnight refresh with a passive milestone component. It does not
compute milestones itself.

