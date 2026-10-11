<!-- intent-skills:start -->

## Skill Loading

Use the repository’s installed Intent. If it is unavailable, report the missing dependency instead of downloading a replacement.
Before editing files for a substantial task:

- Run `bunx --no-install --package @tanstack/intent intent list` from the workspace root to see available local skills.
- If a listed skill matches the task, run `bunx --no-install --package @tanstack/intent intent load <package>#<skill>` before changing files.
- Use the loaded `SKILL.md` guidance while making the change.
- Monorepos: when working across packages, run the skill check from the workspace root and prefer the local skill for the package being changed.
- Multiple matches: prefer the most specific local skill for the package or concern you are changing; load additional skills only when the task spans multiple packages or concerns.

<!-- intent-skills:end -->

# Dashboard guidance

Production TanStack Start dashboard for a private household iPad: glanceable information, home controls and focused feature views.

- Before implementing features, read [release issue #69](https://github.com/patrickhaahr/openhome/issues/69), including its UI clarification, and the relevant feature sub-issue for scope and acceptance criteria.
- Use `../prototypes/executor-dashboard/` as inspiration; develop a cohesive production design with better hierarchy, usability and polish. Reuse components selectively.
- Use shadcn/ui for new UI components; add them from the repo root with `just dashboard-add <component>`. Extend the existing theme tokens consistently.
- Keep rules, data and integrations in the Axum API and credentials on the server.
- Design for iPad touch, landscape and portrait, accessible contrast, reduced motion and older-device performance. Use browser previews and project tests for routine webapp checks. When an attached iPad can resolve a Safari/device-specific issue or a hardware acceptance check, invoke [ipad-control](.agents/skills/ipad-control/SKILL.md) for actual-device verification.
- Use anonymous synthetic data in public code, fixtures and screenshots; keep household identities, coordinates and credentials private.
