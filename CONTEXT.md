# OpenHome

OpenHome is a homelab control system with an Axum API and mobile clients. Mobile clients interact only with the Axum API; the API owns integration with underlying services and devices.

## Language

### Client and Integration Boundaries

**Mobile Client**:
A user-facing app that consumes the Axum API and does not communicate directly with homelab devices or service bridges.
_Avoid_: Direct bridge client, LAN device client

**CLI Client**:
A noninteractive command-line program that consumes the Axum API and does not communicate directly with homelab devices or service bridges.
_Avoid_: Device script, API wrapper

**Agent**:
An automated operator that invokes a CLI Client or OpenHome tools to fulfill a user request.
_Avoid_: Integration Service, direct device agent

**Axum API**:
The only backend boundary that mobile clients call to read state and trigger actions.
_Avoid_: Gateway, proxy layer

**Integration Service**:
A backend component that knows how to talk to a specific homelab service or device bridge behind the Axum API.
_Avoid_: Client app service, mobile adapter

### Client Access

**Base URL**:
The configured root address of the Axum API that the Mobile Client connects to.
_Avoid_: Device address, bridge address

**API Key**:
A shared bearer credential that authorizes a Mobile Client to call the Axum API.
_Avoid_: User token, session token

**Setup Flow**:
A client-side step where the user enters the Base URL and API Key and validates them against the Axum API before first use.
_Avoid_: Account onboarding, server registration

**Native Client Access**:
The access model for `mobile-expo`: complete Setup Flow once, then open directly into the app.
_Avoid_: Biometric gate, locked-by-default startup

**Home Geofence**:
A user-configured circular region centered on the Mobile Client's current location. When the client exits the region, it asks the Axum API to turn off the lights.
_Avoid_: Continuous GPS tracking, direct light automation

**Home Radius**:
The distance in meters from the center of the Home Geofence that defines when the Mobile Client has left home.
_Avoid_: GPS interval, polling distance

### Areas

Each area's terms live in their own file. Read the files for the areas you are changing.

- [Home Controls](docs/context/home-controls.md): IR, Lights, Backlight and mobile navigation terms.
- [Dashboard and Household Lists](docs/context/dashboard.md): The Dashboard, Household Users, Wishlists, Watchlists, Important Dates and route terms.
- [Binder](docs/context/binder.md): Household trading card collections.
- [Server Operations](docs/context/server-operations.md): Server Tab, Docker, AdGuard and Feed terms.
- [Fitness](docs/context/fitness.md): Training, NOOP, recovery and body terms.
- [Nutrition](docs/context/nutrition.md): Food, Product, Meal Log, Supplement and target terms.
