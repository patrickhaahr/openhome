# Image encoding and meal photos

Card images and meal photos pass through one AVIF encoder
(`services/avif_encoder.rs`) on the Raspberry Pi. Meal photos are files beside
the database rather than BLOBs (ADR 0017 explains why card images differ).
These rules bound the encoder's cost and keep files and rows consistent.

- **Bounded decode.** The encoder bounds the source size and decode memory
  against decompression bombs: a 48-megapixel RGBA image plus headroom. A meal
  photo is rejected before decoding when it cannot decode within the maximum
  photo size.
- **One encode at a time.** A process-wide semaphore allows a single encode,
  because the Pi has no CPU to spare. The permit moves into the blocking
  worker, so a caller that is dropped still holds it until the encode ends.
- **Pixels only.** Uploads are JPEG or PNG, recognised by signature, as
  base64 that may be a data URL. They are rotated upright from EXIF and then
  re-encoded from pixels. That drops all metadata (EXIF, GPS, XMP, ICC), and
  the original upload is never written. A CMYK JPEG is refused: the decoder has
  already converted it to RGB, so applying its CMYK profile would mislabel the
  colours.
- **Settings.** Meal photos use aom speed 8 and quality 60 at a 1,600 px long
  side, plus a 400 px thumbnail. That is about 0.6 s per photo on one Pi 5 core
  and about 235 KB per photo.
- **Raw libavif.** The safe libavif wrapper does not expose the CICP colour
  fields, so `avif_encoder/native.rs` uses the C API. Native resources stay in
  that module and are released on every error path. The C API takes a mutable
  pointer even for read-only RGB input, and the encoded data stays valid until
  it is dropped.
- **Body limit only where needed.** Only the nutrition routes that carry a
  photo raise Axum's default body limit, because a base64 photo at the maximum
  size is about 13.4 MiB.
- **Photo lifecycle.** A photo is *Encoded* in memory, then *Stored* as two
  files named by a fresh UUID that no row references, then *Referenced* by the
  committed Meal Log row. The caller owns stored files until its row write
  finishes, and removes them if the write fails or loses a replay race. Files
  are renamed into place before the row commits. A replaced or deleted photo's
  files are removed only after the commit, as best effort, and failures are
  logged. Storing never touches the database, so the log's receipt claim
  happens inside its own transaction.
- **Known gaps, accepted.** Files and SQL share no commit, nothing is fsynced
  and nothing collects orphans. A dropped future or a crash can leave
  unreferenced files, mixed `.partial` and complete files, or a replaced
  photo's old files. Writes go through a temporary name, so a crash never
  leaves a partial file under the real name. Only a UUID can name a file, so no
  path leaves the photo directory.
- **Caching.** Stored images are household data behind the API key, so
  responses are `private` and never shared-cacheable. A meal photo ID always
  names the same bytes, so it is `immutable` for a year. Card images are cached
  for a day. Proxied wishlist pictures are cached for an hour and sent with
  `nosniff` and a sandboxing CSP.
- **Raster only.** The wishlist relay passes only raster formats, because SVG
  can carry script. The declared content type must match the bytes.
