---
name: openhome
description: 'Control OpenHome (the smart home) through the first-party `openhome` CLI. Use for any natural-language OpenHome request: checking health, turning lights on or off, IR remote commands for the Edifier speaker or LG TV, opening TV apps, managing AdGuard Protection (status, enable, disable, timed pause), or setting the TV Backlight (TV Sync, red, any RGB color, off). Examples: "turn off my lights", "make the backlight blue", "pause AdGuard for 30 minutes", "mute the speaker", "open YouTube on the TV".'
metadata:
  hermes:
    category: smart-home
    tags: [openhome, lights, ir, adguard, backlight, smart-home]
    requires_toolsets: [terminal]
---

# OpenHome Control

Control OpenHome through its `openhome` CLI. The CLI is noninteractive, sends
every request to the Axum API with the configured API Key, and prints the API
response as JSON on standard output. Diagnostics go to standard error.

## Rules

- Run `openhome --help` (or `openhome <command> --help`) for the authoritative
  command reference. Do not rely on memory for flags or arguments.
- Never construct raw HTTP requests and never contact Integration Services
  directly. Every OpenHome request goes through `openhome`.
- The API Key and Base URL are already configured in the environment. Do not
  read, print, or pass API Key values on the command line.
- Commands may change real devices and network filtering. No confirmation
  prompt exists; run the command the user asked for and report the JSON result.

## Canonical Commands

```bash
# API health: verifies connectivity and authentication.
openhome health

# Lights.
openhome lights on
openhome lights off

# IR remotes: see which remotes and commands are available.
openhome ir status

# Send a named command to the Edifier speaker or the LG TV.
openhome ir edifier <command>
openhome ir lgtv <command>

# Open a TV app directly on the LG TV by number.
# 1=youtube 2=jellyfin 3=spotify 4=crunchyroll 5=twitch
openhome ir lgtv <1-5>

# AdGuard Protection.
openhome adguard status
openhome adguard enable
openhome adguard disable
openhome adguard pause <minutes>   # resumes automatically

# TV Backlight. Brightness is 0-100; red and color default to 100.
openhome backlight status
openhome backlight tv [--brightness <0-100>]   # TV Sync: follows the screen
openhome backlight red [--brightness <0-100>]
openhome backlight color <rgb> [--brightness <0-100>]
openhome backlight off
openhome backlight automatic   # drop manual overrides, follow Bedtime
```

For `<rgb>`, pass the color the user named as bare hex (`0080ff`) or as
`red,green,blue` (`0,128,255`). Translate a color name into hex yourself. Quote a
leading `#` (`'#0080ff'`), because the shell treats it as a comment.

`backlight tv`, `red` and `color` are manual overrides that apply immediately.
While Bedtime Mode is enabled, an override expires at the next Bedtime start or
end. Use `backlight automatic` when the user asks to reset the Backlight or
return it to normal: it clears the override and applies the current routine
target.

A selection is saved before the Bluetooth write, so the command can succeed
while the device write fails. Before reporting that the Backlight changed, check
`command`, `recovery` and `observed` in the JSON and report a failure if they
show one.

`<command>`, `<minutes>` and `<rgb>` come from the user request; the API decides which
commands are valid (`openhome ir status` lists what is available).
