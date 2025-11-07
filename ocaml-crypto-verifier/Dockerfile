# Multi-stage build for OCaml crypto verifier
FROM ocaml/opam:debian-12-ocaml-5.2 AS builder

# Install system dependencies
RUN sudo apt-get update && sudo apt-get install -y \
    pkg-config \
    libssl-dev \
    && sudo rm -rf /var/lib/apt/lists/*

# Copy source code
WORKDIR /app
COPY ocaml-crypto-verifier/ ./ocaml-crypto-verifier/

# Build the application
WORKDIR /app/ocaml-crypto-verifier
RUN opam install . --deps-only --yes
RUN opam exec -- dune build @install

# Runtime stage
FROM debian:12-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd --create-home --shell /bin/bash app
USER app

# Copy built binary
WORKDIR /home/app
COPY --from=builder /app/ocaml-crypto-verifier/_build/install/default/bin/server ./crypto-verifier

# Expose port
EXPOSE 8001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8001/health || exit 1

# Run the server
CMD ["./crypto-verifier"]
