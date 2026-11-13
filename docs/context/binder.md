# Binder

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

**Binder**:
A Household User's one collection of trading cards, starting with the One Piece
Card Game. The whole household can see every Binder; edits go to its owner. Its
pages show Cards in Card Code order, parallels after their normal print.
_Avoid_: Collection, deck, Wishlist

**Card**:
One printed variant of a trading card as the OPTCG catalogue lists it, with its
image and game details. A normal print and its parallel art are different Cards
that share a Card Code. A card the catalogue does not list cannot be added yet.
_Avoid_: Manual card, product

**Card Code**:
The identifier printed on a card, such as `OP01-001`, shared by all its variants.
_Avoid_: Card ID, set number

**Binder Entry**:
One Card in one language in a Binder, with the quantity owned. Language is
English unless marked Japanese. Adding the same Card in the same language again
raises the quantity rather than adding another entry.
_Avoid_: Copy, slot, condition grade

**Card Price**:
The TCGplayer market price in USD that the OPTCG catalogue publishes for a Card,
with the date the catalogue observed it. It prices English printings, so a
Japanese Binder Entry has no Card Price.
_Avoid_: Live price, Cardmarket price

**Price History**:
The Card Prices OpenHome has observed since the Card was first stored.
A price whose observation date has not changed adds nothing, and nothing is
backfilled or interpolated.
_Avoid_: Market history, daily price series

**Card Value**:
A Card Price shown in the chosen currency (Danish kroner by default, euros or
US dollars), converted at the ECB reference rate for the price's date, or the
latest known rate when that day's is missing.
_Avoid_: Hard-coded rate, appraisal

**Binder Value**:
The sum of quantity times Card Value over a Binder's priced Binder Entries,
stating how many entries have no price.
_Avoid_: Portfolio, net worth
