# 04 — Container logs

**What to build:** Selecting a Container on the Docker Tab swaps the list view for that Container's recent logs — a fixed tail of ~200 timestamped lines fetched in one shot with manual refresh. Going back returns to the Container list.

**Blocked by:** 02 — Container list + Docker Health Summary.

**Status:** done (2493ba3)

- [x] Tapping a Container shows its logs with timestamps included
- [x] Log fetch requests the fixed tail (200) and passes no `since` filter
- [x] Manual refresh re-fetches the logs
- [x] Not-found (404) responses render "container not found"; other failures render inline
- [x] Back returns to the unfiltered container list without stale log state
- [x] Machine tests cover load, refresh, and failure paths against the scripted fake adapter
