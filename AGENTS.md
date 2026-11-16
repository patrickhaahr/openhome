# Repository guidance

- Read `CONTEXT.md` and the `docs/context/` area files it links before changing product behavior or domain terminology.
- Run project commands through `devenv shell -- <command>`.
- Use `just` recipes; run `just` to list them. Add a recipe when a required project command is missing.
- Keep every file under 20,000 bytes, the global pre-commit hook's cap; `just file-size-check` lists offenders. Split by responsibility; `.gitattributes` exemptions are only for generated output and plain data.
- After changes, run the modified areas' `just <area>-check` gates and fix all errors; `just check` runs every area.
- Deploy: read [the deployment skill](.agents/skills/deploy/SKILL.md) when asked to deploy or release this repository to the Pi, including a bare "deploy" request.
- Verify: read [the verification skill](.agents/skills/verify-openhome/SKILL.md) when proving an API, MCP or Dashboard change works on a running stack.
