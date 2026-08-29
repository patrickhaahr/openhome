# 03 — Container lifecycle actions

**What to build:** Each Container row offers start, stop, and restart that execute immediately with no confirmation step. Failures are surfaced inline and distinguished: container not found vs Docker service unavailable vs other errors.

**Blocked by:** 02 — Container list + Docker Health Summary.

**Status:** done (1e01d72)

- [x] Start / stop / restart buttons dispatch instantly — no confirmation dialog of any kind
- [x] Successful action updates the affected Container's state in the list
- [x] Not-found (404) responses render a "container not found" message naming the Container
- [x] Unavailable (503) responses render a "Docker unavailable" message
- [x] Other failures render their error text inline without crashing the list
- [x] Machine tests cover immediate dispatch, success refresh, and each distinct failure class against the scripted fake adapter
