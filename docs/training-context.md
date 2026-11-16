# Training context

The Axum API owns the training read model: calendar days, units, NOOP source resolution, freshness and coverage ([ADR 0002](adr/0002-training-context-api-and-mcp.md)). The Hermes MCP adapter (`mcp/`, see `mcp/AGENTS.md`) sends each tool call to one API endpoint and returns the API's JSON unchanged. It never reads a database and has no rules of its own.

Reads today: the **Training Context**, the **Recovery Day**, **recent Sleep Nights**, **Workouts on a Training Day**, the **NOOP Workout Detail**, **Exercise Set history**, and **metric trends**. The Training Context and NOOP trends share the
[source resolution](training-context-rules.md#source-resolution) and [freshness and coverage](training-context-rules.md#freshness-and-coverage) rules.

Each read and rule lives on its own page.

- [Training context: Training Context and metric trends](training-context-context-and-trends.md) covers the Training Context and metric trends reads.
- [Training context: sleep and recovery](training-context-sleep-and-recovery.md) covers Recent Sleep Nights and the Recovery Day.
- [Training context: workouts](training-context-workouts.md) covers Workouts on a Training Day and Exercise Set history.
- [Training context: NOOP Workout Detail](training-context-noop-workout-detail.md) covers the NOOP Workout Detail read.
- [Training context: shared rules](training-context-rules.md) covers freshness and coverage, source resolution, inspecting rows and known limits.
