# Use the official Rust image
FROM rust:1.72-slim as builder

# Create a new empty shell project
RUN USER=root cargo new --bin copypaste
WORKDIR /copypaste

# Copy the manifests
COPY ./Cargo.toml ./Cargo.toml

# This build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# Copy the source code
COPY ./src ./src

# Build for release
RUN rm ./target/release/deps/copypaste*
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
COPY --from=builder /copypaste/target/release/copypaste /usr/local/bin/copypaste

# Copy static files
COPY ./static /static

# Set the working directory
WORKDIR /

# Set environment variables
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000

# Expose the port the app runs on
EXPOSE 8000

# Run the application
CMD ["copypaste"]
