# Manual Watchlists (#101)

Public captures use only Member A / Member B and fictional movie/series titles,
served through `just dashboard-health-fixture-run` with production authenticated
Axum routes and an isolated migrated in-memory database. The Start server holds
the synthetic API credential; the browser sees only parsed data and safe outcomes.

| Capture                                         | Evidence                                                                                             |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| [Before](before.png)                            | Watchlist placeholder at baseline `412e6c8`, 1180 × 820                                              |
| [Landscape](landscape.png)                      | Separate owner cards, persisted movie/whole-series plans, planned status and added dates, 1180 × 820 |
| [Portrait](portrait.png)                        | Stacked owner cards, readable controls and content, 820 × 1180                                       |
| [Removal confirmation](remove-confirmation.png) | Owner and title named before permanent removal                                                       |

The collaborative T3 browser exercised adding a movie for Member A, adding a
whole series for Member B, confirming removal, and navigating afresh to verify
the remaining persisted plan. The layout has no horizontal overflow at either
capture size. The automated rendered seam additionally verifies failed
credentials, unavailable reads, and an accepted addition whose response is lost,
followed by remount and retry without duplication.

Run `devenv shell -- just api-check mcp-check dashboard-check`. Database rules
are primarily tested through migrated authenticated Axum routes; the MCP seam
uses the real Streamable HTTP client and forwards Axum results unchanged. No
TMDB integration is claimed by this manual slice. These desktop browser captures
do not substitute for the physical iPad Safari release check under #69.
