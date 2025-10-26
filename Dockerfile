# Use the official Rust image with Rust 1.82
FROM rust:1.82-slim as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/copypaste

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Create data directory and set permissions
RUN mkdir -p /app/data /app/static && chown -R 1000:1000 /app

# Copy the binary from the builder stage
COPY --from=builder /usr/src/copypaste/target/release/copypaste /app/

# Copy static files
COPY static /app/static

# Run as non-root user
USER 1000:1000

# Set environment variables
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000
ENV ROCKET_DATABLES={}

# Expose the port the app runs on
EXPOSE 8000

# Command to run the application
CMD ["/app/copypaste"]
