# Multi-stage Dockerfile for copypaste.fyi server and frontend

# 1) Build blockchain tooling (Hardhat)
FROM node:20 AS blockchain-builder
WORKDIR /app/blockchain

COPY blockchain/package*.json ./
RUN npm ci

COPY blockchain ./
RUN npm run build

# 2) Build frontend assets using Node
FROM node:20 AS frontend-builder
WORKDIR /app/frontend

COPY frontend/package*.json ./
RUN npm ci

COPY frontend ./
RUN npm run build

# 3) Build backend binary with Rust
FROM rust:1.84 AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install --no-install-recommends -y pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Pre-fetch dependencies to benefit from caching when the source changes infrequently
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked

# Copy the actual project sources
COPY src ./src
COPY static ./static
COPY docs ./docs
COPY README.md ./
COPY --from=blockchain-builder /app/blockchain ./blockchain

# Build the application binary
RUN cargo build --release --locked --bin copypaste

# Runtime image with only the compiled binary and static assets
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/copypaste /usr/local/bin/copypaste
COPY --from=builder /app/static ./static
COPY --from=frontend-builder /app/frontend/dist ./static/dist

ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000
EXPOSE 8000

CMD ["copypaste"]
