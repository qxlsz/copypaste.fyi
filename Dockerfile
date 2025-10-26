# Use the official Rust image with Rust 1.82
FROM rust:1.82-slim as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/copypaste

# Copy dependency files
COPY Cargo.toml .
COPY Cargo.lock .
COPY migrations ./migrations

# Create dummy source files for initial build
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release

# Copy source code
COPY . .

# Build the application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Create data directory and set permissions
RUN mkdir -p /app/data /app/static && chown -R 1000:1000 /app

# Copy the binary from the builder stage
COPY --from=builder /usr/src/copypaste/target/release/copypaste /app/

# Copy migrations and static files
COPY --from=builder /usr/src/copypaste/migrations /app/migrations
COPY static /app/static

# Run as non-root user
USER 1000:1000

# Set environment variables
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000
ENV DATABASE_URL=sqlite:/app/data/pastes.db
ENV RUST_LOG=info

# Expose the port the app runs on
EXPOSE 8000

# Command to run the application
CMD ["/app/copypaste"]
