# Handoff: LAN Power API

## Goal

Design and implement an authenticated web API on the always-on `pi` host to manage `zaza` and `nika`:

```text
POST /hosts/{host}/wake
POST /hosts/{host}/shutdown
POST /hosts/{host}/reboot
GET  /hosts/{host}/status
```

Only permit the fixed host identifiers `zaza` and `nika`. Never accept arbitrary SSH targets, MAC addresses, or commands from request input.

## API Behavior

- `wake`: send a Wake-on-LAN magic packet to the host's Ethernet MAC through its subnet broadcast address.
- `shutdown`: use restricted SSH credentials to run `systemctl poweroff`.
- `reboot`: use restricted SSH credentials to run `systemctl reboot`.
- `status`: use a bounded SSH connectivity check. Return `online`, `offline`, or `unknown`.

Return `202 Accepted` when a power action is successfully dispatched. Clients can poll the status endpoint to observe completion.

Example response:

```json
{
  "host": "zaza",
  "state": "online"
}
```

Suggested errors:

- `400`: malformed request.
- `401` or `403`: authentication failure.
- `404`: unknown host.
- `409`: action conflicts with known state, if enforced.
- `502`: SSH or dispatch failure.
- `504`: operation timeout.

## Current State

- `pi` runs Debian 13 and is always on.
- `wakeonlan` is installed on `pi`.
- Wake-on-LAN has successfully started `zaza`.
- `zaza` is configured for TPM2-backed unattended LUKS unlocking.
- `zaza` firmware requires `PCIE Devices Power On` enabled and `Deep Sleep` disabled.
- `nika` Ethernet, MAC, firmware WoL support, and shutdown authorization remain to be inspected.
- Treat IP and MAC details as deployment configuration rather than public documentation.

## Repository References

Inspect the current working diff rather than duplicating it here:

```bash
git diff
git status --short
```

Relevant files:

- `modules/aspects/host/zaza-hardware.nix`: `zaza` WoL and TPM2 LUKS configuration.
- `modules/aspects/host/zaza.nix`: `zaza` composition and commented service imports.
- `modules/aspects/identity/ph.nix`: SSH authorized keys.
- `modules/aspects/cli/openssh.nix`: SSH client/server configuration.
- `modules/aspects/security/doas.nix`: privilege elevation.
- `justfile`: remote deployment behavior.
- Motherboard manual: https://download.asrock.com/Manual/H310M-HDV.pdf

Run `just verify <host>` after relevant NixOS changes.

## Security Requirements

- Authenticate every endpoint.
- Keep credentials outside source control in protected runtime secrets.
- Bind to a private network or place the API behind an authenticated private proxy.
- Rate-limit requests and audit action, target, result, and caller without logging credentials.
- Give the API a dedicated SSH identity.
- Restrict that key on each host to a fixed power-command dispatcher. Do not allow an interactive shell or arbitrary commands.
- Allow passwordless privilege elevation only for the required power operations.
- Execute subprocesses with argument arrays and short timeouts, never shell interpolation.
- Use SSH for graceful shutdown. Do not normally cut power with a smart plug.

## Open Decisions

- Decide where the API source belongs; `pi` runs Debian, so do not assume this NixOS repository is the application repository.
- Select the smallest runtime already available on `pi`.
- Define the authentication and network-exposure boundary before implementation.
- Decide whether the API remains stateless or records transitional states such as `waking` and `shutting_down`.
- Discover and test `nika`'s wired NIC, MAC, subnet broadcast, firmware settings, and WoL capability.
- Decide idempotency behavior for waking an online host or shutting down an offline host.

## Suggested Skills

- `codebase-design`: create a narrow host-control boundary without arbitrary command execution.
- `domain-modeling`: define hosts, actions, status states, and dispatch results.
- `diagnosing-bugs`: verify WoL and SSH behavior for each host.
- `write-better-error-messages`: design useful timeout and unreachable-host responses.
- `tdd`: test authentication, host allowlisting, subprocess calls, and timeouts.
- Invoke the language-specific standards skill for the selected implementation language.

## Next Steps

1. Determine the API project location and authentication requirements.
2. Inspect runtimes and deployment conventions on `pi`.
3. Inspect `nika` while online and configure/test WoL.
4. Define a fixed host registry containing SSH target, MAC, and broadcast address.
5. Implement `zaza` status end-to-end first, then its power actions, then add `nika` through the same registry model.
