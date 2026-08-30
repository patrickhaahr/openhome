# 01: Ship the authenticated CLI Client

**What to build:** Deliver a first-party, source-packaged `openhome` CLI Client that can verify Axum API access through a `health` command. It must resolve configuration predictably, authenticate with the existing shared API Key, emit API JSON for callers, and provide the reusable request path for later control commands without changing the Axum API.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `openhome health` sends an authenticated request to the existing health endpoint and writes the successful JSON response to standard output.
- [ ] Base URL precedence is explicit `--url`, then `OPENHOME_API_URL`, then `https://openhome.haahr.me`.
- [ ] API Key precedence is `OPENHOME_API_KEY_FILE`, then `OPENHOME_API_KEY`, then the conventional OpenHome key file under the user's configuration home.
- [ ] Missing, unreadable, and empty API Keys fail before an HTTP request and never appear in diagnostics.
- [ ] HTTP, API, and malformed-response failures write useful diagnostics to standard error and return a failed process status.
- [ ] The CLI is noninteractive and introduces no table output, dry-run mode, confirmation flow, quiet mode, or specialized exit-code taxonomy.
- [ ] Process-level tests invoke the compiled CLI against a local stub server and verify request path, bearer authorization, JSON output, diagnostics, process status, and configuration precedence.
- [ ] The OpenHome flake exposes the CLI as a buildable package using its locked Rust dependencies.
- [ ] Repository commands cover building, running, testing, formatting, and linting the CLI.
- [ ] The domain glossary defines CLI Client and Agent and records that an Agent invokes a CLI Client while all device and service access remains behind the Axum API.
- [ ] Existing Axum API behavior and source remain unchanged.
