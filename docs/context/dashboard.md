# Dashboard and Household Lists

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

## Dashboard

**Dashboard**:
A user-facing OpenHome interface on a dedicated household display, combining
information and home controls with access to focused feature views.
_Avoid_: Home Tab, Training Context

**Dashboard Home**:
The glanceable overview shown by the Dashboard, where selecting a feature opens
its focused view and inactivity returns to the overview.
_Avoid_: Home Tab, permanent widget grid

**Dashboard Remote**:
A Dashboard control surface for one IR Remote. It shows only the Remote Buttons
whose Commands the Axum API currently reports, omitting rather than disabling
the rest. An accepted Command acknowledges the request only; it never
establishes the device's power, input or volume.
_Avoid_: Unavailable Remote Button, device state panel

**Idle Slideshow**:
A full-screen show of photos from the selected Immich albums that covers the
Dashboard after a configured spell of inactivity, with the time, date and
today's weather and Show When Idle dates on one plate. Since dates count elapsed
time, yearly dates rotate nearest first, and the figures re-read at Copenhagen
midnight. Photos from earlier years on this day lead the slideshow and show their
age quietly on the same plate. The first touch or key only wakes it and returns
to the page and state that were showing; it never reaches a control beneath.
_Avoid_: Screensaver, photo gallery, Photos page

**Idle Block**:
A declared reason the Dashboard must not enter the Idle Slideshow: an open
sheet or dialog, an unsaved draft, a save in flight or a conversation.
_Avoid_: Wake lock, busy flag

**Household User**:
A configured person who owns Wishlists, Watchlists and a Binder and can be
tagged on Important Dates. The shared Dashboard
shows household members together within these list views, without separate
passwords or private lists.
_Avoid_: Fitness Profile, authenticated account

**Wishlist**:
A Household User's named collection of items they would like to acquire.
_Avoid_: Shopping checklist, Watchlist

**Wishlist Item**:
A desired item recorded in a Wishlist, described by a generic request or a
specific product with an optional link and supporting details.
_Avoid_: Gift reservation, shopping order

**Loop**:
A generated walking or running route that starts and ends near the shared map
origin, measured from its own geometry and compared with the requested
distance. It becomes the Active Route only when the household selects it.
_Avoid_: Walking Loop, Active Route, fabricated route

**Loop Alternatives**:
Up to three Loops for one request that mostly follow different ways, matching
loops first. Fewer are shown, and the shortfall explained, when the search finds
fewer distinct loops within tolerance; none is invented to fill the set.
_Avoid_: Route variants, padded results

**Step Target**:
A Loop target given in steps and converted to distance with the household's
adjustable step length. Step counts shown beside a Loop are estimates from that
step length, never measurements.
_Avoid_: Step tracking, measured steps

**Active Route**:
The selected walking or running route made available to the Dashboard and the
Mobile Client for viewing with the phone's current location.
_Avoid_: GPX transfer, route screenshot

**Route Session**:
A started walk or run on the Mobile Client that keeps its chosen route until
the user accepts a replacement or ends the session.
_Avoid_: Workout, NOOP Workout

**Watchlist**:
A Household User's collection of movies and series planned for watching,
currently being watched or completed, with dates recording addition and completion.
_Avoid_: Wishlist, viewing history import

**Watchlist Entry**:
One movie or whole series on a Household User's Watchlist, with a planned,
watching or completed status and addition/completion dates.
_Avoid_: Episode log, Wishlist Item

**Important Date**:
A household-shared date the household counts from or towards, with a title and
a kind (Since or Yearly). It is never owned: it tags the Household Users it
concerns, both members for a shared anniversary and none for a friend's
birthday. Axum computes its figures for a Europe/Copenhagen day.
_Avoid_: Reminder, calendar event, owned date

**Since**:
An Important Date kind that counts up from the date: whole years, months and
days elapsed, plus the total in days.
_Avoid_: Countdown, age

**Yearly**:
An Important Date kind that counts down to the date's next anniversary. In a
common year, 29 February falls back to 28 February.
_Avoid_: Recurring reminder, repeat rule

**Milestone**:
An Axum-computed celebration on an Important Date's Copenhagen day: a Since
anniversary, 100 or 500 days, each thousand days, or a Yearly occurrence.
Anniversaries take precedence over round day counts. Only dates with Show When
Idle enabled may supply the idle screen's leading Milestone.
_Avoid_: Reminder, client-computed celebration, notification

**Show When Idle**:
An Important Date's switch allowing the shared idle photo screen, which anyone
in the room can see, to show it. Off by default.
_Avoid_: Private date, pinned date

**Shared Viewing Progress**:
Viewing state reported by the household's shared Jellyfin account, without
attributing it to an individual Household User.
_Avoid_: Personal completion history, Watchlist status
