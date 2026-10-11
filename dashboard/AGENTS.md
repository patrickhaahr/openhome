# Dashboard guidance

Production TanStack Start dashboard for a private household iPad: glanceable information, home controls and focused feature views.

- Before implementing features, read [release issue #69](https://github.com/patrickhaahr/openhome/issues/69), including its UI clarification, and the relevant feature sub-issue for scope and acceptance criteria.
- Use `../prototypes/executor-dashboard/` as inspiration; develop a cohesive production design with better hierarchy, usability and polish. Reuse components selectively.
- Use shadcn/ui for new UI components; add them from the repo root with `just dashboard-add <component>`. Extend the existing theme tokens consistently.
- Keep rules, data and integrations in the Axum API and credentials on the server.
- Design for iPad touch, landscape and portrait, accessible contrast, reduced motion and older-device performance. Verify physical iPad/Safari behavior separately from desktop previews.
- Use anonymous synthetic data in public code, fixtures and screenshots; keep household identities, coordinates and credentials private.
