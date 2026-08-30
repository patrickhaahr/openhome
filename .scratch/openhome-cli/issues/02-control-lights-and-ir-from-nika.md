# 02: Control lights and IR from Nika

**What to build:** Let `ph` on Nika control the lights and IR Remotes through the new CLI Client, with the API Key supplied by Nika's own SOPS configuration. Replace the generated per-command executables with direct CLI invocations while retaining reliable speaker input changes during startup and shutdown.

**Blocked by:** 01: Ship the authenticated CLI Client.

**Status:** done

- [ ] `openhome lights on` and `openhome lights off` call the existing light endpoints and emit their JSON responses.
- [ ] `openhome ir status` emits the existing IR Remote status response.
- [ ] `openhome ir edifier <command>` and `openhome ir lgtv <command>` send the named non-empty Command to the corresponding existing endpoint.
- [ ] IR Command values are passed through to the Axum API rather than maintained as a duplicate CLI enum.
- [ ] Process-level tests verify methods, paths, bearer authorization, JSON bodies, successful output, API errors, and rejected missing Command values.
- [ ] Nika consumes the pinned OpenHome flake package and makes `openhome` available to `ph`.
- [ ] Nika's own SOPS configuration renders the shared API Key as a user-readable OpenHome key file without placing its value in the Nix store.
- [ ] Every existing Niri light and IR binding invokes the new command hierarchy directly.
- [ ] Startup Bluetooth and shutdown optical automations invoke the new command hierarchy directly and preserve their current retry and timeout behavior.
- [ ] The generated light and IR command executables and their obsolete wiring checks are removed.
- [ ] Nix evaluation checks verify the package, Niri bindings, lifecycle commands, retries, timeouts, and SOPS ownership without reading or printing the API Key.
- [ ] The relevant OpenHome and Nika verification commands pass.
