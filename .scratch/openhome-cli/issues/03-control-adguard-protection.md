# 03: Control AdGuard Protection

**What to build:** Let a CLI Client inspect, enable, disable, and temporarily pause AdGuard Protection through the existing Axum API, using the same configuration, authentication, JSON output, and error behavior established by the health command.

**Blocked by:** 01: Ship the authenticated CLI Client.

**Status:** done

- [ ] `openhome adguard status` calls the existing status endpoint and emits its JSON response.
- [ ] `openhome adguard enable` enables AdGuard Protection through the existing endpoint and emits the resulting JSON status.
- [ ] `openhome adguard disable` disables AdGuard Protection through the existing endpoint and emits the resulting JSON status.
- [ ] `openhome adguard pause <minutes>` sends the requested duration through the existing Protection Pause endpoint and emits the resulting JSON status.
- [ ] Invalid or missing numeric pause input fails without sending an HTTP request; Axum API validation remains authoritative for domain limits.
- [ ] All AdGuard commands remain noninteractive and require no confirmation flag.
- [ ] Process-level tests verify methods, paths, bearer authorization, pause JSON, successful output, API errors, and process status for every AdGuard action.
- [ ] No AdGuard Integration Service or Axum API behavior is changed.
- [ ] CLI tests, formatting, and linting pass.
