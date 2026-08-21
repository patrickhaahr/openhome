# 02 — Container list + Docker Health Summary

**What to build:** The Docker Tab lists Containers with name, image, run state, health status, uptime, ports, and restart count, filterable by health classification with per-classification counts. The Server Tab gains a Docker Health Summary card classifying all Containers as healthy / unhealthy / idle (none running) / offline (Axum API unreachable), showing the running-vs-total count and jumping to the Docker Tab on tap. Includes the tolerant wire-contract parsing and the longer request timeout for docker calls.

**Blocked by:** 01 — AdGuard Protection control (Server Tab shell and refresh convention exist).

**Status:** ready-for-agent

- [ ] Docker Tab renders every Container from the list endpoint with state, health, image, uptime, ports, and restart counts
- [ ] Health filter chips (all/healthy/unhealthy/stopped) filter the list; counts shown per classification
- [ ] Parser tolerates both PascalCase and snake_case field spellings and normalizes numeric uptime given as string
- [ ] Summary card shows healthy/unhealthy/idle/offline classification plus "N of M running"
- [ ] Tapping the summary card switches to the Docker Tab
- [ ] Offline (fetch failure) renders as its own summary state, distinct from idle
- [ ] Docker requests use the extended timeout while other adapter calls keep the short one
- [ ] Container-list parsing tested through a stubbed fetch layer; machine orchestration tested against the scripted fake adapter
