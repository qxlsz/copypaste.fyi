#!/bin/bash
set -e

# Create necessary directories
mkdir -p data

# Build and run the application
docker-compose up --build -d

echo "Pastebin service is starting..."
echo "Access the application at http://localhost:8000"
