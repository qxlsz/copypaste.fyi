#!/bin/bash
# Local CI script for OCaml crypto verifier
# Run this from the ocaml-crypto-verifier directory

set -e

echo "🚀 Starting local OCaml CI..."

# Check if opam is available
if ! command -v opam &> /dev/null; then
    echo "❌ opam not found. Please install opam first."
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "crypto_verifier.opam" ]; then
    echo "❌ crypto_verifier.opam not found. Run this script from ocaml-crypto-verifier directory."
    exit 1
fi

echo "📦 Installing dependencies..."
opam install . --deps-only --yes --with-test

echo "🔨 Installing Dune..."
opam install dune --yes

echo "🏗️  Building project..."
opam exec -- dune build

echo "🧪 Running tests..."
opam exec -- dune test

echo "🐳 Building Docker image..."
docker build -t crypto-verifier-test .

echo "🧪 Testing Docker container..."
docker run -d --name crypto-verifier-test -p 8001:8001 crypto-verifier-test
sleep 5

# Test health endpoint
if curl -f http://localhost:8001/health; then
    echo "✅ Health check passed!"
else
    echo "❌ Health check failed!"
    docker stop crypto-verifier-test
    docker rm crypto-verifier-test
    exit 1
fi

# Cleanup
docker stop crypto-verifier-test
docker rm crypto-verifier-test

echo "🎉 Local OCaml CI completed successfully!"
