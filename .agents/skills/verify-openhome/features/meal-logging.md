# Meal logging

Hermes searches Foods, logs Meal Logs as Portions (optionally with a photo) and reads the Nutrition Day. The Dashboard's Food page shows the day.

## Sub-features

- `food-search` finds Foods by name through `search_foods`.
- `meal-log` logs Portions through `log_meal` with an explicit `eaten_at` and `request_id`.
- `nutrition-day` reads totals through `nutrition_day`.
- `food-dashboard` shows the logged meal on the Food page.

## How to get to it (user POV)

- Send Hermes what you ate, or a photo of it (MCP tools `search_foods`, `log_meal`, `nutrition_day`).
- Dashboard: `Main navigation` → `Food`.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`, and the preview's invented Food database is loaded (`search_foods` returns results).

- **Search.** Run `mcp.sh call search_foods '{"query":"<seed food name>"}'` and take a `food_id`.
- **Log.** Run `mcp.sh call log_meal '{"eaten_at":"<today>T12:00:00+01:00","portions":[{"food_id":"<id>","grams":150}],"request_id":"verify-<run id>-meal"}'`. The result has the Meal Log and `replayed: false`.
- **Read back.** Run `mcp.sh call nutrition_day '{"day":"<today>"}'`. Totals include the portion.
- **Dashboard.** Open path `/food`, wait for the food's name, snapshot.

## Gotchas

- Not yet driven end to end with this skill. Confirm argument names against `mcp.sh tools/list` on the first run, then update this file.
- Photos are base64 JPEG/PNG up to 10 MB. A photo encode runs one at a time, so a large photo makes the call slow, not hung.
