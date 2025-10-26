# Build stage
FROM rust:1.75 as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/copypaste

# Copy only the files needed for building dependencies
COPY Cargo.toml ./
COPY src/ src/

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
RUN mkdir -p /app/data && chown -R 1000:1000 /app/data

# Copy the binary from the builder stage
COPY --from=builder /usr/src/copypaste/target/release/copypaste /usr/local/bin/copypaste

# Copy static files and migrations
COPY static /app/static
COPY migrations /app/migrations

# Run as non-root user
USER 1000:1000

# Set environment variables
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:/app/data/pastes.db

# Expose the port the app runs on
EXPOSE 8000

# Command to run the application
CMD ["copypaste"]

# Command to run the application
CMD ["/usr/local/bin/copypaste"]
