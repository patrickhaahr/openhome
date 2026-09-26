# Training context belongs to the API

OpenHome will build the combined training read model and bounded read endpoints in the Axum API. A separate MCP adapter will call those endpoints and expose typed, read-only tools to Hermes; it will not query either SQLite database or interpret fitness metrics. This keeps date, unit, provenance, and coverage rules in one place for later API clients. The adapter uses the existing `API_KEY`, and the existing deployment restricts network access to the tailnet.
