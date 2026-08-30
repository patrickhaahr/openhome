Status: ready-for-agent

# Spec: OpenHome CLI for agent and human control

## Problem Statement

The homelab owner and trusted Agent currently need generated shell wrappers or raw HTTP requests to control OpenHome. Those interfaces are difficult to discover, duplicate authentication and request details, and do not give an Agent a stable command vocabulary for requests such as turning off the lights or pausing AdGuard Protection. The CLI must be available to `ph` on Nika and to Hermes on Zaza without changing the Axum API or exposing secrets through the Nix store.

## Solution

Add a first-party **CLI Client** named `openhome`. It calls the existing Axum API with the existing shared API Key and exposes a small, discoverable command hierarchy for health, lights, IR Remote Commands, and AdGuard Protection. It is noninteractive and returns API JSON on standard output so humans and Agents can consume the same interface.

Package the CLI through the OpenHome flake and consume that pinned package from `nixos-config`. Nika and Hermes retain separate SOPS configurations containing the same API Key: Nika exposes a user key file, while Hermes receives the key through its existing SOPS-managed environment. A declarative Hermes skill teaches the Agent when and how to invoke the CLI.

## User Stories

1. As a homelab owner, I want one `openhome` executable, so that I do not need to remember raw HTTP routes.
2. As a homelab owner, I want command help to list available capabilities, so that I can discover the interface without separate documentation.
3. As a homelab owner, I want to check Axum API health, so that I can verify connectivity and authentication.
4. As a homelab owner, I want to turn the lights on from the CLI, so that I can control them without opening a Mobile Client.
5. As a homelab owner, I want to turn the lights off from the CLI, so that I can control them without opening a Mobile Client.
6. As a homelab owner, I want to inspect IR Remote status, so that I can see which remotes and Commands are available.
7. As a homelab owner, I want to send a named Edifier Command, so that I can control the speaker through the Axum API.
8. As a homelab owner, I want to send a named LG TV Command, so that I can control the television through the Axum API.
9. As a homelab owner, I want the CLI to accept any non-empty IR Command name, so that the Axum API remains authoritative when available commands change.
10. As a homelab owner, I want to inspect AdGuard Protection status, so that I know whether filtering is running and enabled.
11. As a homelab owner, I want to enable AdGuard Protection, so that network filtering can be restored quickly.
12. As a homelab owner, I want to disable AdGuard Protection, so that I can troubleshoot network behavior.
13. As a homelab owner, I want to start a Protection Pause for a specified number of minutes, so that filtering resumes automatically.
14. As a homelab owner, I want every successful command to emit JSON, so that I can inspect or pipe results consistently.
15. As a homelab owner, I want failures written to standard error with a failed process status, so that scripts can distinguish success from failure.
16. As a homelab owner, I want the public OpenHome deployment used by default, so that normal commands require no Base URL setup.
17. As a developer, I want to override the Base URL with an environment variable, so that I can target a development Axum API.
18. As a developer, I want an explicit command-line Base URL to override every other source, so that one invocation can target a chosen Axum API.
19. As a developer, I want to provide an API Key directly through an environment variable, so that local and automated development remain simple.
20. As a Nika user, I want the CLI to read my SOPS-managed key file automatically, so that the API Key is not stored in shell history or the Nix store.
21. As a Hermes operator, I want Hermes to receive the same API Key from its existing SOPS-managed environment, so that its established secret workflow remains intact.
22. As a Hermes operator, I want the CLI installed in the Hermes runtime environment on Zaza, so that Agent terminal commands can invoke it.
23. As a Hermes operator, I want an OpenHome skill available to Hermes, so that natural-language home-control requests map to the CLI.
24. As a Hermes operator, I want the skill to use CLI help as its detailed command reference, so that skill instructions do not drift from the executable.
25. As a Hermes operator, I want requests such as “turn off my lights” to invoke `openhome lights off`, so that ordinary language can control OpenHome.
26. As a Hermes operator, I want AdGuard requests to use the same CLI, so that Hermes does not construct raw HTTP requests.
27. As a trusted operator, I want commands to run without confirmation prompts, so that Agent and systemd execution never blocks on interactive input.
28. As a Nika user, I want existing desktop shortcuts to invoke the first-party CLI directly, so that generated compatibility executables can be removed.
29. As a Nika user, I want speaker boot and shutdown automations to retain their retry behavior, so that transient network readiness does not break them.
30. As a Zaza operator, I want the CLI installed without Nika's speaker lifecycle automations, so that Zaza startup or shutdown does not change speaker input.
31. As a NixOS operator, I want `nixos-config` to pin the OpenHome source revision, so that deployments are reproducible.
32. As a NixOS operator, I want to update OpenHome through the flake lock, so that no separate binary-download or checksum workflow is needed.
33. As a maintainer, I want the CLI to remain a thin Client, so that business rules and Integration Service behavior stay in the Axum API.
34. As a maintainer, I want the Axum API contract and authorization behavior left unchanged, so that existing Mobile Clients continue working.

## Implementation Decisions

- Add a standalone Rust crate for the CLI Client rather than adding another binary to the Axum API crate. The crates remain independently buildable and the CLI does not compile server-only dependencies.
- Use Clap derive for the command hierarchy, blocking Reqwest with Rustls for HTTP, Serde JSON for request bodies, and Anyhow for application-level error context. Do not add an async runtime, configuration framework, table renderer, or server-type dependency.
- The command surface is `health`; `lights on|off`; `ir status`; `ir edifier <command>`; `ir lgtv <command>`; and `adguard status|enable|disable|pause <minutes>`.
- IR Command names are non-empty strings passed to the Axum API. The CLI does not mirror the API's available-command list as an enum.
- Existing Axum API methods, routes, payloads, response shapes, and bearer authorization remain unchanged. The CLI sends the configured API Key as an Authorization bearer credential.
- Base URL precedence is explicit `--url`, then `OPENHOME_API_URL`, then `https://openhome.haahr.me`. An explicit flag ignores every other Base URL source.
- API Key precedence is `OPENHOME_API_KEY_FILE`, then `OPENHOME_API_KEY`, then the conventional OpenHome key file under the user's configuration home. Missing, unreadable, and empty credentials are reported before a request is attempted.
- Successful API response bodies are written as JSON to standard output. Diagnostics and API failure details are written to standard error. Process status communicates success or failure without a specialized exit-code taxonomy.
- The CLI is always noninteractive. It has no confirmation, `--yes`, `--dry-run`, `--quiet`, table-output, or TTY-sensitive behavior.
- Apply one finite HTTP timeout to prevent Agent and systemd calls from hanging indefinitely. Preserve Nika's existing outer retry loop for boot and shutdown speaker commands.
- Expose the CLI as the OpenHome flake's package for supported systems. `nixos-config` consumes the OpenHome flake input and follows its existing Nixpkgs input, while its lock file pins the source revision.
- The OpenHome NixOS module installs and configures the CLI independently from optional desktop lifecycle automations. Nika enables both; Zaza enables the CLI but not the automations.
- Install the package in Zaza's system environment so the `hermes` user and Hermes Agent terminal environment can execute it.
- Remove the generated per-command shell executables. Update every known Niri and systemd consumer to invoke `openhome` with arguments directly.
- Nika and Hermes keep separate encrypted secret sources containing the same trusted API Key. Nika renders a user-owned OpenHome key file. Hermes adds `OPENHOME_API_KEY` to its existing SOPS-managed environment.
- Hermes remains fully trusted with the shared API Key. No API key scopes, additional keys, route restrictions, or authorization changes are introduced.
- Add an OpenHome local skill to the declarative shared skills source. Configure Hermes to discover that source as an external skill directory. The skill requires terminal capability, directs home-control and AdGuard requests through `openhome`, and uses `openhome --help` rather than duplicating the full reference.
- Add **CLI Client** and **Agent** to the domain glossary. A CLI Client consumes the Axum API; an Agent invokes a CLI Client to fulfill a user request. This documentation does not change API authorization behavior.
- No ADR is warranted because the command surface, credential lookup, and source-built Nix packaging are reversible decisions.

## Testing Decisions

- Good CLI tests assert observable process behavior rather than private functions: given arguments and an isolated environment, verify the outgoing HTTP request and the resulting standard output, standard error, and process status.
- Use one primary high-level seam: invoke the compiled CLI Client against a local stub HTTP server. At this seam, verify request method, path, bearer authorization, JSON body, successful JSON output, API errors, transport errors, and failed status.
- Cover every command family through the primary process seam, with representative cases for both light states, IR status and command dispatch, all AdGuard Protection actions, and Protection Pause payloads.
- Verify Base URL precedence at the process seam: explicit `--url` overrides the environment, the environment overrides the production default, and the production default is selected only when both overrides are absent.
- Verify API Key precedence and validation at the process seam using isolated environment variables and temporary user configuration. Tests must not depend on real credentials or the production Axum API.
- Keep argument-parser-only tests to cases rejected before HTTP, such as missing subcommands, missing IR Command values, and invalid numeric Protection Pause input. Do not duplicate Clap's own behavior exhaustively.
- Nix checks verify that the OpenHome flake builds an executable package and that the configured Nika and Zaza closures contain it.
- `nixos-config` evaluation checks verify that Nika's Niri bindings and lifecycle services invoke the new command hierarchy, retain retry and timeout wiring, and that Zaza does not enable Nika-only automations.
- Nix checks verify the declared SOPS ownership and runtime wiring without decrypting or printing either API Key.
- Skill checks verify valid skill frontmatter, discoverability from Hermes's configured external skill directory, terminal requirement metadata, and the presence of canonical light and AdGuard guidance.
- Prior art is the repository's existing Axum integration tests for high-level HTTP behavior and `nixos-config`'s existing evaluated service/package checks for OpenHome wiring.

## Out of Scope

- Changes to the Axum API, Integration Services, routes, payloads, responses, or authentication middleware.
- API key scopes, separate Agent credentials, user accounts, sessions, login, or key rotation workflows.
- Docker management, Feeds, Compact Timeline, item read state, and random facts.
- Table output, adaptive TTY output, quiet mode, dry runs, confirmation prompts, and specialized exit-code categories.
- Interactive setup, persistent CLI configuration files, shell completion generation, and credential storage performed by the CLI.
- MCP servers, dedicated Agent execution layers, or direct communication with homelab devices and service bridges.
- GitHub release binaries, cross-platform binary archives, and binary caches introduced specifically for this CLI.
- Compatibility wrappers for the previous generated command names.
- Changes to deprecated clients.

## Further Notes

- The existing API Key grants Hermes access to API capabilities not exposed by the v1 CLI. This is accepted because Hermes is fully trusted; the CLI command surface is not treated as an authorization boundary.
- The Base URL is not secret. API Keys must never be embedded in Nix derivations, command-line arguments, test fixtures committed to source control, or diagnostic output.
- The OpenHome skill is procedural guidance, not another integration layer. Hermes still reaches devices and services only through the CLI Client and Axum API.
