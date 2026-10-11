# Agent wishlist round trip — #97

Recorded 2026-10-11 by the implementing agent acting as Hermes, per the
[user clarification](https://github.com/patrickhaahr/openhome/issues/97#issuecomment-6113664863).
The real API and MCP executables ran through `just api-run` and `just mcp-run`
inside devenv, using an isolated file-backed SQLite database with all migrations.
An authenticated MCP Streamable HTTP session invoked the actual tools. Household
Users were provisioned through the authenticated API. All data below is synthetic.

Before this change the MCP endpoint exposed seven read-only training tools and
had no wishlist persistence or tools. After this change it additionally supports
owner/list discovery, named-list creation, generic addition and item reads.

| Action | Observed result |
| --- | --- |
| `household_users()` before provisioning | `[]`, `isError:false` |
| Provision and discover owners | `Member A` and `Member B`, each with a stable ID |
| `create_wishlist(owner_id: Member A's ID, name: "Outdoors")` | A named list with its owner ID, `isError:false` |
| `wishlists()` | The new `Outdoors` list |
| `add_wishlist_item(title: "A warm jacket", request_id: "agent-demo-1")` | `isError:true`, status 409, `wishlist_destination_required`, one question and list choices; no write |
| Add with the explicit `Outdoors` list ID | `isError:false`, `replayed:false`, title `A warm jacket` |
| Repeat identical addition with the same request ID | Same item ID and timestamp, `replayed:true` |
| `wishlist_items(list_id: Outdoors ID)` | Exactly one generic item, `A warm jacket` |
| Reuse the request ID with title `A different wish` | `isError:true`, status 409: ID already used for a different addition |
| Restart the API against the same database, then read/retry via MCP | Same single item, stable ID and timestamp; retry remains `replayed:true` |

The accepted item returned through MCP was:

```json
{
  "item": {
    "created_at": "2026-10-11T21:01:03.513Z",
    "id": "c0580331-f039-47af-a985-1f8184f2e9a1",
    "list_id": "be181a9b-f9d0-4a97-87b4-835e49299540",
    "title": "A warm jacket"
  },
  "replayed": false
}
```

No product research or URL fetching occurred. This verifies the agent/MCP
workflow for #97; dashboard editing and rich/URL items belong to later sub-issues.
Automated API and real MCP contract coverage additionally exercises destination
defaults and changes, owner/list mismatch, concurrent retries, input validation,
authentication and failed-write propagation. Existing training contracts pass
alongside the additive wishlist tools.
