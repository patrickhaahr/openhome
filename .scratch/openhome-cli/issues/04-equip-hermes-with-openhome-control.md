# 04: Equip Hermes with OpenHome control

**What to build:** Give the trusted Hermes Agent on Zaza the packaged CLI Client, its shared API Key through Hermes's existing SOPS-managed environment, and a declarative OpenHome skill that maps natural-language light, IR, health, and AdGuard requests to noninteractive CLI commands.

**Blocked by:** 02: Control lights and IR from Nika; 03: Control AdGuard Protection.

**Status:** ready-for-agent

- [ ] Zaza consumes the pinned OpenHome flake package and the `hermes` user can execute `openhome` through the Hermes terminal environment.
- [ ] The OpenHome NixOS module can install the CLI independently from Nika-only lifecycle automations.
- [ ] Zaza enables the CLI while leaving the startup Bluetooth and shutdown optical automations disabled.
- [ ] Hermes receives `OPENHOME_API_KEY` from its existing SOPS-managed `.env`, using the same trusted API Key as Nika without exposing it through Nix or logs.
- [ ] Hermes can use the default production Base URL without additional configuration while retaining support for an environment override.
- [ ] A declarative OpenHome skill is included in the shared local skill source and discovered by Hermes through its configured external skill directories.
- [ ] The skill activates for natural-language OpenHome, light, IR Remote, and AdGuard Protection requests and requires terminal capability.
- [ ] The skill directs Hermes to use `openhome --help` for command details and never to construct raw HTTP requests or contact Integration Services directly.
- [ ] The skill includes canonical guidance for turning lights on and off, sending an IR Command, checking health, and managing AdGuard Protection.
- [ ] Skill validation verifies frontmatter, discovery configuration, terminal metadata, and canonical command guidance.
- [ ] Nix evaluation verifies that Hermes receives the executable and skill while Zaza remains free of Nika-only automations.
- [ ] The Zaza verification command passes without decrypting or printing the API Key.
