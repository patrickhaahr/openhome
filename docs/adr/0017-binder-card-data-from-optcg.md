# Binder card data comes from the OPTCG catalogue

Axum takes Card details, images and prices from one source, the OPTCG
catalogue at optcgapi.com. It is free, needs no key and answers one Card Code
request with every variant: the details, an image URL, the TCGplayer market
price in USD and the date the catalogue observed it. Variant identity is the
catalogue's image identifier, treated as opaque (`OP01-001`, `OP04-016_P1`,
`EB03_OP09-034_p1`). The Card Code comes from the variant's own field, and each
image is fetched from the catalogue's `card_image` URL on the configured origin.

OpenHome records its own Price History. The catalogue keeps no usable history
(only a two-week window), so OpenHome stores a Price Point whenever it sees a
newer observation date, starting from the day a Card is first stored. Nothing
is backfilled or interpolated, and a price whose date has not changed adds
nothing. Each price keeps its observation date because some variants go months
without a refresh.

Card images are stored as AVIF in the main database. Axum fetches each image
once (the catalogue serves PNGs labelled `.jpg`), re-encodes it with the shared
AVIF encoder at full size and writes it in the same transaction as the Card
row. There are no image files to place, orphan or clean up, and the Dashboard
and Hermes read images from OpenHome, never from the volunteer-run server.

## Considered Options

- Cardmarket: matching its products to individual card variants is unreliable,
  and its price files are undocumented.
- TCGplayer's own API: closed to new developers.
- Hot-linking the catalogue's images: every Dashboard view would burden one
  volunteer's paid server and depend on it being up.
- Image files beside the database, as meal photos have: a second thing to back
  up, and a window where a row and its file disagree. Card images are small,
  immutable and always written with their Card, so a BLOB keeps them atomic.

## Consequences

The catalogue is a single point of failure for new Cards only. Stored Cards,
their images and their Price History keep working when it is down, and a
failed lookup writes nothing and answers `card_data_unavailable` (503) without
upstream text. The adapter has a fixed production origin, a short timeout, a
response size cap and no redirects, and asks for a Card Code once, then once
per owned Card Code per day, paced, because the catalogue publishes no rate
limit and asks callers to be modest. A code it does not list is asked again at
most once per Copenhagen day.

Prices are TCGplayer's English-printing market prices, so Japanese Binder
Entries have none. The main database grows by one image per stored Card and is
backed up with it. Card names and images are trademarks of their owners, and
the Dashboard credits the catalogue as the data source.

Card Values in DKK and EUR come from the ECB's daily reference rates, read from its
published XML file by an adapter with the same limits as the catalogue's, so that
prices need no paid service. The file is parsed with `quick-xml`, a real XML parser,
strictly, and a DTD is refused. See [`binder-values.md`](../binder-values.md).
