# Build from the repo root: docker buildx build -f mcp/Dockerfile .
# The api crate is a dev-dependency, so cargo needs its manifest and sources to resolve the workspace path.
FROM --platform=$BUILDPLATFORM rust:1.91-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu gcc-x86-64-linux-gnu \
        libc6-dev-arm64-cross libc6-dev-amd64-cross \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc

WORKDIR /build

COPY api/Cargo.toml ./api/Cargo.toml
COPY api/src ./api/src
COPY mcp/Cargo.toml mcp/Cargo.lock ./mcp/
COPY mcp/src ./mcp/src

WORKDIR /build/mcp
ARG TARGETARCH
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/mcp/target \
    case "$TARGETARCH" in \
        amd64) target=x86_64-unknown-linux-gnu ;; \
        arm64) target=aarch64-unknown-linux-gnu ;; \
        *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && cargo build --release --locked --target "$target" \
    && cp "target/$target/release/openhome-mcp" /build/openhome-mcp

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 appuser

COPY --from=builder /build/openhome-mcp /usr/local/bin/openhome-mcp

USER appuser

EXPOSE 8001

CMD ["openhome-mcp"]
