# Important Dates

Household-shared Since and Yearly dates with Axum-computed figures and Milestones. Hermes adds dates with explicit Household User tags, and the Dashboard's Lists → Dates view shows them.

## Sub-features

- `date-add-mcp` adds a date through `add_important_date` with `person_ids` (required, may be empty).
- `date-dashboard` shows the date and its computed figures in the Dates view.

## How to get to it (user POV)

- Ask Hermes to remember a date (MCP tools `important_dates`, `add_important_date`).
- Dashboard: `Main navigation` → `Lists` → the `Dates` tab, and the `Add a date` button.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`.

- **Add.** Run `mcp.sh call add_important_date '{"title":"Verify anniversary","kind":"since","date":"2020-01-01","person_ids":[],"request_id":"verify-<run id>-date"}'`. The result has the stored date and its figures.
- **Read back.** Run `mcp.sh call important_dates '{}'`. The list includes `Verify anniversary`.
- **Dashboard.** Open path `/lists?tab=dates`, wait for text `Verify anniversary`, snapshot.

## Gotchas

- Not yet driven end to end with this skill. Confirm argument names against `mcp.sh tools/list` and labels against a fresh snapshot on the first run, then update this file.
- Figures are computed for the Europe/Copenhagen day. A run near midnight UTC can disagree with a local expectation.
