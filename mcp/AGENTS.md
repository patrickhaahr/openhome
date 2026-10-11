# MCP guidance

- Read `../docs/training-context.md` before changing training tools or their contracts.
- Keep training tools read-only: each call makes one authenticated API GET and passes its JSON unchanged. Add an API endpoint before adding a tool that needs new interpretation; database access and domain rules belong in the API.
- Percent-encode path arguments, reject literal `.` and `..` segments, and keep API redirects disabled so requests stay on the intended endpoint.
