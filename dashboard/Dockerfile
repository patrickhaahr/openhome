FROM oven/bun:1.4.2 AS builder

WORKDIR /app

COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

COPY . .
RUN bun --bun x vite build

FROM oven/bun:1.4.2-slim AS runtime

WORKDIR /app
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000

COPY --from=builder --chown=bun:bun /app/.output ./.output

USER bun
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD bun -e 'const r = await fetch("http://127.0.0.1:3000/connection"); process.exit(r.ok ? 0 : 1)'

CMD ["bun", ".output/server/index.mjs"]
