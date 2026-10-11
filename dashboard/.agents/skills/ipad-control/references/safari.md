# Actual Safari inspection

## Serve the dashboard over USB

1. From the OpenHome repository, start a synthetic health fixture with
   `devenv shell -- just dashboard-health-fixture-run`, then run the dashboard
   with that fixture's printed URL and synthetic API key through
   `devenv shell -- env ... just dashboard-run`. Use the existing `just` recipes
   and keep the actual API credentials server-side. Read the server's printed
   port: Vite may increment it when a port is occupied.
2. Check the host listener's address family with `ss -ltnp` or `curl`. The
   verified Vite listener used IPv6 loopback `[::1]:3001`; an IPv4 destination
   silently made the first tunnel unusable. Adapt this reverse SSH tunnel to
   the current listener, keeping it in a managed background session:

   ```sh
   SSHPASS="$IPAD_SSH_PASSWORD" sshpass -e ssh -F /dev/null \
     -o ConnectTimeout=5 -o ExitOnForwardFailure=yes \
     -o UserKnownHostsFile=/tmp/openhome-ipad-known-hosts \
     -N -R '127.0.0.1:3000:[::1]:3001' -p 23244 "$IPAD_SSH_USERNAME"@127.0.0.1
   ```

   On the iPad, run `/cores/binpack/usr/bin/curl -I http://127.0.0.1:3000`
   through SSH. Continue when the device receives HTTP 200, then use
   `/var/jb/usr/bin/uiopen --url http://127.0.0.1:3000` through SSH to open Safari.

## Attach Web Inspector

1. Ask the user to enable **Settings → Safari → Advanced → Web Inspector**
   if needed, and leave the dashboard tab visible. Rotate the physical iPad
   when checking another orientation; a viewport override is emulation.
2. Locate `ios_webkit_debug_proxy` with `command -v` or the installed Nix store.
   Start `ios_webkit_debug_proxy -F` with output redirected to a private `/tmp`
   log. Its default device directory is port 9221; the first device's page
   directory is `http://127.0.0.1:9222/json`. Select only the dashboard URL;
   other Safari tabs and device identifiers may be private. Continue when a
   matching page has a `webSocketDebuggerUrl`.
3. If the handshake reports `SSL broken pipe`, verify Web Inspector and an
   open Safari tab, then restart the proxy. Repeating SSH authentication or
   installing another inspector is unnecessary when SSH already works. A page
   listed in the directory can still time out while Safari is inactive: verify
   the iPad is awake with the dashboard visible and only one inspector attached,
   then restart the proxy once if needed. Report a persistent timeout rather
   than treating directory discovery as a successful inspection.

If the proxy is missing, obtain it from the host's Nix packages. On this host,
`ios-webkit-debug-proxy` 1.9.2 initially failed to build because GCC promoted a
discarded-qualifier warning to an error. This temporary package override built
successfully without changing the repository:

```sh
nix build --impure --no-link --print-out-paths --expr \
  'let pkgs = import <nixpkgs> {}; in pkgs.ios-webkit-debug-proxy.overrideAttrs (old: { NIX_CFLAGS_COMPILE = (old.NIX_CFLAGS_COMPILE or "") + " -Wno-error=discarded-qualifiers"; })'
```

Use the resulting `bin/ios_webkit_debug_proxy`; discover store paths rather
than caching hashes. `idevicescreenshot` and `idevicedebug` could not start
their services without a developer disk image on this iPad. Web Inspector's
page snapshots worked and exclude browser chrome.

## Run inspector commands

[../scripts/inspect.mjs](../scripts/inspect.mjs) accepts a JSON command array
and the exact dashboard page URL. From the repository root, run
`devenv shell -- just dashboard-ipad-inspect /tmp/ipad-commands.json
http://127.0.0.1:3000/`. The optional third argument selects another device's
page directory; the default is `http://127.0.0.1:9222/json`.

Example command file:

```json
[
  { "method": "Console.enable" },
  {
    "method": "Runtime.evaluate",
    "params": {
      "expression": "JSON.stringify({path:location.pathname,viewport:{width:innerWidth,height:innerHeight},overflow:document.documentElement.scrollWidth>innerWidth,heading:document.getElementById('page-heading')?.textContent,focus:document.activeElement?.id,clock:document.querySelector('time')?.innerText})",
      "returnByValue": true
    }
  },
  {
    "method": "Page.snapshotRect",
    "params": { "x": 0, "y": 0, "width": 768, "height": 921, "coordinateSystem": "Viewport" },
    "saveImage": "/tmp/ipad-dashboard.png"
  }
]
```

Read the current viewport before choosing snapshot dimensions. For navigation,
activate an observed dashboard link with `Runtime.evaluate`, then add
`{ "delay": 500 }` before measuring layout/focus; Safari scrolling also settles
asynchronously. The helper fails on inspector errors and thrown evaluations,
saves PNGs without logging their base64, and reports console warnings/errors.

Modern iOS sends `Target.targetCreated` first. Dispatch commands inside
`Target.sendMessageToTarget` and decode `Target.dispatchMessageFromTarget`.
Sending `Runtime.evaluate` directly fails with **Runtime domain was not found**;
Chrome DevTools Protocol is not a substitute. The helper handles this wrapper.

Verify only the checks relevant to the change: layout in real orientations,
navigation/history/focus, independent rail/content scrolling, errors or clock
rollover. For enlarged text, set `document.documentElement.style.fontSize` to
`200%`, measure after settling, then remove the property. Record this as text
enlargement; Safari pinch/page zoom and finger gestures require physical input.
