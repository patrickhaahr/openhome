# Minimal runtime image for pre-built binary
# Build binary on dev machine with:
#   cross build --release --target aarch64-unknown-linux-musl

FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    curl

WORKDIR /app

# Copy pre-built binary (cross-compiled for ARM64)
COPY openhome-api /app/openhome-api
RUN chmod +x /app/openhome-api

# Copy migrations for SQLx runtime migrations
COPY migrations /app/migrations

# Create data directory for SQLite databases
RUN mkdir -p /app/data

EXPOSE 8000

CMD ["/app/openhome-api"]
