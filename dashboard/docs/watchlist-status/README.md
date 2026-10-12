# Watchlist status, dates and filters (#102)

Public captures use Member A / Member B and fictional titles, served by the
production authenticated Axum routes over an isolated migrated in-memory database.
Start holds the synthetic API credential on the server.

| Capture                           | Evidence                                                                               |
| --------------------------------- | -------------------------------------------------------------------------------------- |
| [Before](before.png)              | Manual planned entries at baseline `272a973`, 1180 × 820                               |
| [Landscape](landscape.png)        | Shared planned/watching/completed entries and owner/status filters, 1180 × 820         |
| [Portrait](portrait.png)          | Stacked owner cards and inline status/date controls, 820 × 1180                        |
| [Date correction](edit-dates.png) | Explicit owner/status filter and editable added/completion dates for a completed movie |

The collaborative T3 browser changed Member A’s whole series to watching and
completed Member B’s movie, verifying the default completion date. Owner/status
filters survived a fresh page navigation. Correcting the movie to added
2024-02-28 and completed 2024-02-29 persisted both dates in the rendered readback.
Neither layout overflowed horizontally; visible controls measured at least 48 px
high. The automated rendered seam additionally verifies draft retention after a
rejected credential, retry, remount, empty filtered lists and clearing completion
when moving back to planned.

The API suite covers every status transition for both media kinds, strict date
validation, date-only corrections, explicit null, owner targeting, combined
filters and upgrade of existing plans/receipts. The real MCP contract verifies
partial edits preserve omitted fields, null is forwarded distinctly, query
values are encoded and API errors pass through unchanged. Training tools keep
their read-only contracts.

`devenv shell -- just api-check mcp-check dashboard-check` passes. Physical iPad
Safari checks remain part of release #69; these desktop captures do not substitute
for that hardware verification. TMDB search/metadata stays in #103.
