# NOOP — On-Device Data Model

NOOP is a standalone, fully offline companion app for WHOOP straps (4.0 and 5.0). It talks to
the user's own strap directly over Bluetooth Low Energy — no WHOOP cloud or account
is involved, and stores everything it decodes locally in a single SQLite database.
This document describes that on-device database: every table, its columns, natural keys, indexes,
and the migration history that produced the current schema.

> **Scope note.** Interacting with the strap here means interoperating with the user's *own*
> device and the data it has already recorded. NOOP is **not affiliated with, endorsed by, or
> connected to WHOOP**, and it is **not a medical device** — none of the stored values are
> intended for diagnosis or treatment.

---

The tables are split across three pages.

- [NOOP data model: storage and migrations](DATA_MODEL_STORAGE.md) covers where the database lives, the schema overview and the migration history.
- [NOOP data model: devices, streams and outbox](DATA_MODEL_STREAMS.md) covers the device registry, the decoded streams and the raw outbox.
- [NOOP data model: bookkeeping and caches](DATA_MODEL_CACHES.md) covers cursors, metric caches, the Oura archive, the index summary and provenance.
