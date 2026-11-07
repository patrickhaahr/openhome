# Native Android acceptance

Epic #226's scoped native app passed independent physical release acceptance on
6 November 2026. Native Kotlin Android is now the supported mobile client; Expo
is deprecated for new development, with its source/build recipes retained.
No API/Dashboard functionality or existing fitness data was removed.

## Accepted artifact and journey

- Source: `522509028dc9d3890036d06b6ee097f8288a5865`.
- Pixel 8, Android 17 / GrapheneOS; signed non-debuggable release 0.1.0 (1),
  package `com.patrickhaahr.openhome`.
- APK SHA256: `20b57ac2abccbbf94a6e3a899e9ae032dd2e06a055778e7583fb6363a370da3e`.
- Isolated real Axum, invented credentials/data, loopback ADB forwarding. No
  household controls, real fitness records or controller capture were used.

The physical journey downloaded 620 Workouts, 540 dated weights and 33 Exercises.
With Axum stopped, it created an Exercise, repeated and adjusted a Workout,
confirmed sets, relaunched without losing work, omitted unchecked sets on finish,
corrected History and saved Profile and Body Weight. On API recovery, the proxy
dropped one committed response for each of five mutation kinds. Independent API
and database reads confirmed one new Workout, Exercise and weight, correct Exercise
references and values, and the final Profile revision, without duplicate creates.

Home fixtures exercised TV/full remote, Speaker, explicit Lights, Backlight and
advanced bedtime/profile tools, with honest acknowledgements and errors. Relaunch
and recovery did not replay failed Home commands. Nested Android Back, the 150s
default rest, remembered duration, deadline across relaunch, skip and visible expiry
passed. Real text/numeric keyboards and form layout were inspected on the Pixel.

![Physical release with invented fixture Workout and rest timer](../assets/native/workout-timer.png)

## Checks and deliberate limits

All host recipe gates passed: API, MCP, Expo, native lint/build, firmware, CLI,
Dashboard and deployment checks. Signed release lint/build, alignment, signature,
manifest and package verification passed. Narrow API receipt/revision integration
tests and existing app tests supplied complementary contract evidence.

The current emulator inventory contained 64 tests. Five setup cases passed before
device-support shutdown/refresh failed. The user explicitly waived the remaining
59 in favor of physical release acceptance. Earlier focused passing test batches
were reused against unchanged code; this is not a claim of one green full suite.
Earlier timing/assertion failures were preserved, diagnosed and followed by passing
affected reruns, rather than silently counted as successes.

Physical cellular-only routing, felt vibration, a physical edge-swipe gesture,
exhaustive TalkBack/device/font-size coverage, upgrade from an existing native
release database, and live household hardware remain unclaimed. Prior emulator
cellular-default and gesture tests cover those software policies. Keeping Wi-Fi
for wireless ADB and stopping the fixture proves outage/recovery, not cellular
routing. No app data was cleared. The Pixel retains invented fixture data;
its temporary API is stopped.

The independent PR review subsequently found a repaired refused Workout could
precede a newly created Exercise in the outbox and stall. This was reproduced on
the original physical release against isolated Axum. The fix appends the replacement
create after its dependencies inside the existing save transaction. A second physical
release journey confirmed automatic Exercise-then-Workout delivery, correct identifiers
and sets, and exactly one created Workout without an extra Sync action or relaunch.
The existing app-level correction regression now includes this dependency case;
it was compiled, while runtime verification used the Pixel rather than instrumentation.
Native lint/build and signed-release verification passed after the repair.

## Reproducing checks

Use [`mobile/README.md`](../mobile/README.md) for signed builds and installation.
Run project recipes through devenv. Host gates can run without instrumentation:

```sh
SQLX_OFFLINE=true devenv shell -- just check-api check-mcp mobile-lint mobile-build check-firmware check-cli check-dashboard deployment-check
```

`just mobile-check` also invokes instrumentation when an emulator is booted.
For future full device runs, use memory-safe groups with fresh device-tool boots
between them and archive results before each subsequent run overwrites the XML:

```sh
devenv shell -- just mobile-test SetupFlowTest
```

Never run data-clearing instrumentation on a physical phone. Real-Axum acceptance
uses `mobile-fixture-api`, `mobile-fixture-seed` and `mobile-fixture-lossy-proxy` with
throwaway databases and invented credentials. Check recipe arguments before use.
API code/SQL changes require affected API checks and query metadata regeneration;
offline SQLx checking does not replace maintaining that metadata.
