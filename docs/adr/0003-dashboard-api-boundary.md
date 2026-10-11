# Dashboard integrations belong to the Axum API

The Dashboard reuses OpenHome capabilities and adds features that may also be
used by mobile clients and the Agent. Axum owns integrations and business rules;
TanStack Start serves the dashboard and keeps backend credentials server-side.
We chose this boundary over implementing independent integrations in the
dashboard server so that provider handling and domain rules stay shared across
clients, at the cost of extending the Rust API for new dashboard capabilities.
