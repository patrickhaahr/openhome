# Anti-slop provenance

- Installed from `/home/ph/.agents/skills/install-anti-slop/assets/anti-slop`
  using the bundled `scripts/install.mjs` on 2026-10-05.
- Source repository and exact upstream commit: unknown. The local skill has no
  Git metadata; no upstream revision is inferred.
- Pristine snapshot: all files in this directory except this provenance record
  are byte-for-byte copies of the bundled assets, verified with
  `git diff --no-index` before adding this record. Preserve these files in Git
  as the baseline for future vendored updates.
- Snapshot SHA-256: `03d28831f8f669f4241e5f49534a6026c7f9d5f4987b3082bb9ddeeaa8ffd646`.
  Computed from the source with:

  ```sh
  tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    -cf - -C /home/ph/.agents/skills/install-anti-slop/assets anti-slop | sha256sum
  ```

- Generic entry point: `tools/oxlint/anti-slop/index.ts`.
- Optional Effect entry point: `tools/oxlint/anti-slop/effect/index.ts`, copied
  but not enabled because the project has no direct Effect dependency.
- Intentional deviations from copied source: none. This record is the only
  added file. Vendored source is excluded from project lint and formatting.
- Nested ESLint Stylistic license and provenance are retained in
  `vendor/eslint-stylistic/LICENSE` and `vendor/eslint-stylistic/UPSTREAM.md`.
  That record identifies its own upstream revision and adaptations.

Oxlint and `@oxlint/plugins` are both pinned to `1.86.0`.
