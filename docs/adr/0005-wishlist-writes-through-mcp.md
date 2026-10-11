# Wishlist tools can mutate through the API

Agent-driven Wishlist creation is a primary workflow, so OpenHome's MCP adapter
will expose writable Wishlist tools alongside its read-only training tools.
Training contracts and API ownership from ADR 0002 remain intact: new tools call
Axum endpoints, and the adapter does not access databases or interpret domain
data. This extends the server's original read-only scope instead of creating a
second mutation store; implementation must update global guidance and per-tool
annotations to distinguish reads from writes.
