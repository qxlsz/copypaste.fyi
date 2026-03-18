# syntax=docker/dockerfile:1.4

# ─── Stage 1: Build frontend ──────────────────────────────────────────────────
FROM node:20-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
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

# ─── Stage 3: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/copypaste /usr/local/bin/
COPY --from=builder /app/static /app/static

WORKDIR /app
VOLUME /data
EXPOSE 8000

ENV ROCKET_ADDRESS=0.0.0.0 \
    ROCKET_PORT=8000 \
    COPYPASTE_SQLITE_PATH=/data/copypaste.db

CMD ["copypaste", "serve"]
