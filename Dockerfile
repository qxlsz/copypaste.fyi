# syntax=docker/dockerfile:1.4

# ─── Stage 1: Build frontend ──────────────────────────────────────────────────
FROM node:20-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
COPY frontend/scripts/ ./scripts/
RUN npm ci --prefer-offline
COPY frontend/ ./
RUN npm run build

# ─── Stage 2: Build Rust binary ───────────────────────────────────────────────
FROM rust:1.84-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
# Copy built frontend assets for embedding in static/
COPY --from=frontend /app/frontend/dist ./static/dist/
COPY static/index.html ./static/
# Cache deps layer
RUN cargo build --release --locked --bin copypaste
# Prepare /data with correct ownership for distroless nonroot (UID 65532)
RUN mkdir -p /data && chown 65532:65532 /data

# ─── Stage 3: Runtime (distroless — no shell, runs as nonroot UID 65532) ─────
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder --chown=65532:65532 /app/target/release/copypaste /usr/local/bin/copypaste
COPY --from=builder --chown=65532:65532 /app/static /app/static
COPY --from=builder /data /data

WORKDIR /app
VOLUME /data
EXPOSE 8000

ENV ROCKET_ADDRESS=0.0.0.0 \
    ROCKET_PORT=8000 \
    COPYPASTE_SQLITE_PATH=/data/copypaste.db

# distroless/nonroot sets USER 65532 by default; no shell available so
# Docker-native HEALTHCHECK is omitted — rely on Fly.io HTTP health checks
# configured in fly.toml [[services.http_checks]].
CMD ["/usr/local/bin/copypaste", "serve"]
