# Repository guidance

- Read `CONTEXT.md` before changing product behavior or domain terminology.
- Run project commands through `devenv shell -- <command>`.
- Use `just` recipes; run `just` to list them. Add a recipe when a required project command is missing.
- After changes, run the modified areas' `just <area>-check` gates and fix all errors; `just check` runs every area.
