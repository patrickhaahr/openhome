# 05 — Compact Timeline (read-only)

**What to build:** The Server Tab gains the Compact Timeline: newest-first Feed items paginated via `before_id` as the list end is reached, each item opening its link in the system browser.

**Blocked by:** 01 — AdGuard Protection control (Server Tab shell exists).

**Status:** ready-for-agent

- [ ] Timeline renders compact items newest-first from the timeline endpoint's compact view
- [ ] Reaching the end of the list loads the next page using `before_id`; no duplicates and correct ordering across pages
- [ ] Tapping an item opens its link in the system browser
- [ ] Load-more failures surface inline and don't drop already-rendered items; retry resumes pagination at the right position
- [ ] Empty timeline shows an empty state rather than an error
- [ ] Machine tests cover initial load, pagination, stale-response guarding, and failure paths against the scripted fake adapter
