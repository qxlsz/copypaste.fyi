# Use the official Rust image with a newer version
FROM rust:1.82-slim as builder

# Create working directory
WORKDIR /app

# Copy source code first
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/copypaste /usr/local/bin/copypaste

# Create static directory and copy static files
RUN mkdir -p /static
COPY --from=builder /app/static /static

# Set the working directory
WORKDIR /

# Set environment variables
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000

# Expose the port the app runs on
EXPOSE 8000

# Run the application
CMD ["copypaste"]
