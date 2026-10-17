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

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY noop_migrations ./noop_migrations
COPY .sqlx ./.sqlx

ENV SQLX_OFFLINE=true

ARG TARGETARCH
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    case "$TARGETARCH" in \
        amd64) target=x86_64-unknown-linux-gnu ;; \
        arm64) target=aarch64-unknown-linux-gnu ;; \
        *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && cargo build --release --locked --target "$target" \
    && cp "target/$target/release/openhome-api" /app/openhome-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl gosu \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

WORKDIR /app

COPY --from=builder /app/openhome-api /app/openhome-api
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN mkdir -p /app/data \
    && chown -R appuser:appuser /app \
    && chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 8000

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["/app/openhome-api"]
