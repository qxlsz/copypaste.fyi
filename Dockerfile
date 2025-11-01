# Multi-stage Dockerfile for copypaste.fyi server
FROM rust:1.84 AS builder
WORKDIR /app

# Pre-fetch dependencies to benefit from caching when the source changes infrequently
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked

# Copy the actual project sources
COPY src ./src
COPY static ./static
COPY docs ./docs
COPY README.md ./

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

ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000
EXPOSE 8000

CMD ["copypaste"]
