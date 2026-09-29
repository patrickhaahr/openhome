# Build from the repo root: docker buildx build -f mcp/Dockerfile .
# The api crate is a dev-dependency, so cargo needs its manifest and sources to resolve the workspace path.
FROM rust:1.88-bookworm AS builder

WORKDIR /build

COPY api/Cargo.toml ./api/Cargo.toml
COPY api/src ./api/src
COPY mcp/Cargo.toml mcp/Cargo.lock ./mcp/
COPY mcp/src ./mcp/src

WORKDIR /build/mcp
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 appuser

COPY --from=builder /build/mcp/target/release/openhome-mcp /usr/local/bin/openhome-mcp

USER appuser

EXPOSE 8001

CMD ["openhome-mcp"]
