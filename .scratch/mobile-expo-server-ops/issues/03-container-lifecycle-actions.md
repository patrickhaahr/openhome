# 03 — Container lifecycle actions

**What to build:** Each Container row offers start, stop, and restart that execute immediately with no confirmation step. Failures are surfaced inline and distinguished: container not found vs Docker service unavailable vs other errors.

**Blocked by:** 02 — Container list + Docker Health Summary.

**Status:** ready-for-agent

- [ ] Start / stop / restart buttons dispatch instantly — no confirmation dialog of any kind
- [ ] Successful action updates the affected Container's state in the list
- [ ] Not-found (404) responses render a "container not found" message naming the Container
- [ ] Unavailable (503) responses render a "Docker unavailable" message
- [ ] Other failures render their error text inline without crashing the list
- [ ] Machine tests cover immediate dispatch, success refresh, and each distinct failure class against the scripted fake adapter
