# OpenHome

OpenHome is a homelab control system with an Axum API and mobile clients. Mobile clients interact only with the Axum API; the API owns integration with underlying services and devices.

## Language

### Client and Integration Boundaries

**Mobile Client**:
A user-facing app that consumes the Axum API and does not communicate directly with homelab devices or service bridges.
_Avoid_: Direct bridge client, LAN device client

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

**Unlock Flow**:
A client-side step used by some Mobile Clients to restore access to the stored API Key before the user can use the app.
_Avoid_: Server login, account sign-in

**Setup Flow**:
A client-side step where the user enters the Base URL and API Key and validates them against the Axum API before first use.
_Avoid_: Account onboarding, server registration

**Native Client Access**:
The access model for `mobile-native`: complete Setup Flow once, then open directly into the app without a launch-time Unlock Flow.
_Avoid_: Biometric gate, locked-by-default startup

**Initial Native Tabs**:
The first visible top-level tabs in `mobile-native`: `Home` and `Remote` only.
_Avoid_: Placeholder tabs, future empty destinations

### IR Control

**IR Remote**:
A user-facing control surface that exposes named infrared commands through the Axum API.
_Avoid_: IR device, bridge, blaster

**Command**:
A named IR action that the user triggers from an IR Remote.
_Avoid_: Action, button press, event

### Mobile Navigation

**Home Tab**:
A mixed overview tab that combines the most-used capabilities from multiple API areas.
_Avoid_: Dashboard endpoint, root route group

**Top-Level Tab**:
A primary mobile navigation destination that intentionally mirrors one Axum API area.
_Avoid_: Arbitrary feature bucket, backend implementation detail

**Remote Button**:
A client-defined control in the Remote tab that maps to exactly one IR Command.
_Avoid_: Dynamic API button, raw endpoint action

**Remote Button Set**:
A curated subset of IR Commands exposed by the Android client as Remote Buttons.
_Avoid_: Full command dump, auto-generated control list

**V1 Remote Button Set**:
The initial Remote Button Set for the single IR Remote: `power`, `bluetooth`, `optical`, `mute`, `volume-up`, and `volume-down`.
_Avoid_: Implicit default buttons, test-only commands

**Unavailable Remote Button**:
A Remote Button that remains visible in the layout but is disabled because its Command is not currently reported as available by the Axum API.
_Avoid_: Hidden button, speculative action

**Home Remote Controls**:
A small subset of the Remote Button Set shown on the Home Tab for quick access to the most-used IR Commands.
_Avoid_: Full remote, duplicate tab

**V1 Home Remote Controls**:
The initial Home Remote Controls: `bluetooth` and `optical`.
_Avoid_: Full remote shortcut set, implicit defaults

## Relationships

- A **Mobile Client** calls the **Axum API** for every user-visible capability.
- The **Axum API** delegates device-specific work to one or more **Integration Services**.
- A **Mobile Client** uses a configured **Base URL** to reach the **Axum API**.
- A **Mobile Client** authorizes requests to the **Axum API** with an **API Key**.
- A user completes the **Setup Flow** before first use of the **Mobile Client**.
- Some **Mobile Clients** use an **Unlock Flow** before the stored **API Key** can be used.
- `mobile-native` uses **Native Client Access**.
- `mobile-native` starts with **Initial Native Tabs** only.
- An **IR Remote** exposes one or more **Commands**.
- A **Mobile Client** triggers **IR Remote** **Commands** by calling the **Axum API**.
- A **Home Tab** combines summaries and shortcuts from multiple **Top-Level Tabs**.
- Each **Top-Level Tab** intentionally aligns to one Axum API area.
- A **Remote Button** maps to exactly one **Command**.
- The Android client owns the **Remote Button Set** for the single **IR Remote**.
- The **V1 Remote Button Set** is a fixed client-defined subset of IR Commands.
- An **Unavailable Remote Button** stays visible but disabled when its **Command** is absent from `available_commands`.
- The **Home Tab** preloads IR state for both **Home Remote Controls** and the full **IR Remote** tab.
- **Home Remote Controls** are a subset of the **Remote Button Set**.
- The **V1 Home Remote Controls** are `bluetooth` and `optical`.

## Example dialogue

> **Dev:** "Should the Android app call the IR bridge directly when it is on the same LAN?"
> **Domain expert:** "No. The **Mobile Client** only talks to the **Axum API**. Direct bridge communication is not part of the client model."

## Flagged ambiguities

- "feature tab" and "API endpoint category" were both used for top-level navigation. Resolved: the app uses **Top-Level Tabs** that intentionally mirror Axum API areas, while **Home Tab** is the mixed overview.
- The IR API may expose more commands than the Android app shows. Resolved: the Android app uses a curated **Remote Button Set**, and the `test` command is not exposed.
- "test endpoint" was used to refer to IR behavior. Resolved: the current API has a `test` **Command**, not a separate endpoint.
- "auth" could mean either backend authorization or client re-entry behavior. Resolved: **API Key** names the Axum credential, and **Unlock Flow** names the client-side step to regain access to the stored credential.
- The native Android migration scope is clientside only. Resolved: the backend contract, endpoint shapes, and auth model stay unchanged while the **Mobile Client** is reimplemented natively.
- "same auth flow" was used ambiguously. Resolved: `mobile-native` preserves the backend **API Key** contract and **Setup Flow**, but does not use the old launch-time **Unlock Flow**.
- The tab roadmap is incremental. Resolved: `mobile-native` only shows **Initial Native Tabs** that are actually implemented, starting with `Home` and `Remote`.
