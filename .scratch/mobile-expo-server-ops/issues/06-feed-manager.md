# 06 — Feed manager

**What to build:** An inline collapsible section on the Server Tab manages Feeds: add by pasting a URL (client-validated), delete, and undo a deletion that re-adds the deleted Feeds. Deleting a Feed removes its items from the Compact Timeline; undo restores them.

**Blocked by:** 05 — Compact Timeline (timeline must be visible so delete/undo effects are observable).

**Status:** ready-for-agent

- [ ] Collapsible "manage feeds" section lists current Feeds with delete buttons
- [ ] Adding a valid URL creates a Feed and refreshes both the feed list and the timeline
- [ ] Invalid URLs are rejected with a clear message before any request is sent
- [ ] Delete removes the Feed immediately and refreshes the timeline
- [ ] Undo re-adds the deleted Feeds (batch) and refreshes the timeline
- [ ] Create/delete failures surface inline without losing the entered URL
- [ ] Machine tests cover add, delete, batched undo, and failure paths against the scripted fake adapter
