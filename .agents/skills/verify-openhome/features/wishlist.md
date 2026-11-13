# Wishlist

Household Users keep named Wishlists of items they'd like. Hermes adds wishes to an explicit list or owner, and the Dashboard's Lists → Wish view shows them per member.

## Sub-features

- `wish-add-mcp` adds an item through `add_wishlist_item` with a `request_id`.
- `wish-ambiguous` adds for an owner with several lists and gets one clarification question with choices, with nothing written.
- `wish-dashboard` shows the item in the Wish view.

## How to get to it (user POV)

- Ask Hermes to add a wish (MCP tools `wishlists`, `add_wishlist_item`, `wishlist_items`).
- Dashboard: `Main navigation` → `Lists` → the `Wish` tab, and the `Add a wish` button.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`.

- **Find lists.** Run `mcp.sh call wishlists '{}'` and pick an owner's list ID.
- **Add.** Run `mcp.sh call add_wishlist_item '{"owner_id":"<id>","list_id":"<list>","title":"Verify Lantern","request_id":"verify-<run id>-wish"}'`. The result names the stored item with `replayed: false`.
- **Read back.** Run `mcp.sh call wishlist_items '{"owner_id":"<id>","list_id":"<list>"}'`. The result includes `Verify Lantern`.
- **Dashboard.** Open path `/lists?tab=wish`, wait for text `Verify Lantern`, snapshot.

## Gotchas

- Not yet driven end to end with this skill. Confirm argument names against `mcp.sh tools/list` and accessible names against a fresh snapshot on the first run, then update this file.
- URL metadata previews (`wishlist_url_metadata`) fetch real pages. Use only user-supplied, synthetic or loopback URLs.
