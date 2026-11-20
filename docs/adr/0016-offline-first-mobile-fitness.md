# Keep mobile fitness usable offline

Persist workout logging, exercise creation, workout-history edits, profile changes and body-weight recording on the phone before synchronizing with the Axum API over Wi-Fi. The user needs workouts to survive app closure and unavailable connectivity, accepting the added local persistence and synchronization work rather than relying on API availability during training. Downloaded history, previous performance and the library remain available offline; the product assumes one user and one active client, so it omits multi-client conflict-resolution UI but still requires duplicate-safe synchronization retries.

## Constraints

- **Local commit first.** Every fitness change commits to Room together with its
  queued outbox row in one transaction, so the UI shows it at once whatever the
  network does, and a refusal or rollback changes nothing. Sync scheduling runs
  after the commit and again at app start, in case the process died in between.
- **Local keys.** Room primary keys are local and all references use them;
  `server_id` holds the Axum id once known. A record can therefore exist, and be
  referenced, before Axum has assigned an id. Workout entries resolve an
  Exercise's server id at send time, since that Exercise may still be queued.
  Linking a local Exercise to a server id that another local row already holds
  merges the two rows.
- **Frozen payloads.** An outbox payload is fixed when queued, so a retry repeats
  the same arguments under the same `request_id`. Later edits are later rows. The
  exception is a refused create, which Axum never accepted: it is replaced in
  place, and a repaired Workout create goes after any Exercises queued since, so it
  cannot overtake them. Refused Exercise and Body Weight retries take a new
  `request_id`, because the old one may hold a stale receipt. Merging two local
  Exercises rewrites the merged id in queued Workout payloads.
- **Whole-state edits with revisions.** Profile edits and Workout corrections send
  the whole state with a client revision; Axum applies one only if it is newer than
  the last applied. A newer queued edit supersedes an older one even while the older
  is in flight, because the revision stops the older from taking effect. Request
  models have no defaults, so `null` is sent and clears a value. Downloaded records
  keep Axum's revision, so later local edits still outrank it.
- **Ordered delivery.** `FitnessOutbox` delivers in queue order and stops at the
  first change that may still succeed, so dependents never overtake. A refused
  change is set aside for the user and only its dependents wait. Each row ends in
  one `Delivery`; acknowledgement and row deletion commit together, otherwise the
  row is resent unchanged. A correction for a Workout deleted elsewhere (404) is
  refused rather than retried, so it cannot hold up the queue.
- **Schema durability.** Local fitness data may exist nowhere else, so every Room
  schema change ships with a migration. Queued rows outlive app updates, so a
  payload may only gain optional fields, and each new kind of change gets a new
  `OutboxKind` value instead of reusing one.
- **Wi-Fi only.** Sync sends only when the default network has the Wi-Fi transport
  (`WifiNetworks`); mobile data stays out even when unmetered. The pass also binds
  its requests to Wi-Fi, so an in-flight request fails with that Wi-Fi rather than
  silently moving to mobile data. The Base URL is often a home address, so a VPN
  over Wi-Fi counts and internet validation is not required; a VPN over mobile data
  does not count.
- **WorkManager.** Sync runs through WorkManager because it needs no Google Play
  Services and survives process death and reboot. Its `UNMETERED` constraint only
  approximates Wi-Fi below API 28, so `FitnessSync` checks the transport itself. A
  new change restarts the pass so retry backoff never delays it; replacing a pass
  in flight is safe because a delivered row is re-acknowledged by its `request_id`.
  The in-flight row marker is memory-only so it cannot outlive its sender.
- **Paging the download.** Axum's list endpoints return one page (default 100,
  clamped to 500). History is newest first and Body Weight oldest first, so
  `OpenHomeApi` walks them with inclusive date boundaries because several rows can
  share a date, and stops on a page that adds nothing new. A date holding more rows
  than one page fails with `HistoryCannotBePaged` rather than losing rows.
  Exercises download after Workouts so every referenced Exercise is listed, and
  Workouts insert oldest first so local ids follow server order within a day.
- **Setup is atomic.** Setup downloads everything and commits it in one transaction
  with `COMPLETE` status, so an interruption leaves neither partial data nor a
  completion claim; a stopped `DOWNLOADING` attempt saved nothing and a retry
  restarts. `/api/health` sits behind the API Key check, so a success proves both
  address and key. The API Key is sealed with an AES key held in Android Keystore
  (`ApiKeyCipher`) and may become unreadable after key invalidation.
