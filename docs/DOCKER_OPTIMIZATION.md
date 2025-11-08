# Docker Build Optimization Guide

## Current Optimizations ✅

### 1. **Layer Caching Strategy**
- **Rust**: Dependencies are built separately from source code
  - `Cargo.toml` and `Cargo.lock` copied first
  - Dummy `main.rs` created to build dependencies
  - Source code copied only after dependencies are cached
  - Result: Dependencies only rebuild when `Cargo.lock` changes

- **OCaml**: Similar dependency caching
  - `dune-project` and `.opam` files copied first
  - Dependencies installed before source code
  - Result: Dependencies only rebuild when `.opam` changes

### 2. **Multi-Stage Builds**
- Separate build stages for Rust, OCaml, and runtime
- Only final binaries copied to runtime image
- Reduces final image size significantly

### 3. **Improved .dockerignore**
- Excludes `.git`, `.github`, test files, logs, IDE files
- Reduces Docker context size
- Faster context transfer to Docker daemon

## Additional Speed Improvements

### 4. **Use BuildKit** (Recommended)
Enable Docker BuildKit for parallel builds and better caching:

```bash
# One-time setup
export DOCKER_BUILDKIT=1

# Or add to ~/.zshrc or ~/.bashrc
echo 'export DOCKER_BUILDKIT=1' >> ~/.zshrc
```

Build with BuildKit:
```bash
DOCKER_BUILDKIT=1 docker build -f Dockerfile.backend -t copypaste-backend .
```

### 5. **Use Docker Cache Mounts** (Advanced)
Add cache mounts to Dockerfile for even faster builds:

```dockerfile
# In Rust builder stage
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --bin copypaste

# In OCaml builder stage  
RUN --mount=type=cache,target=/home/opam/.opam \
    opam install . --deps-only --yes
```

### 6. **Use GitHub Actions Cache**
Already configured in `.github/workflows/ci.yml`:
- Caches cargo registry
- Caches target directory
- Caches npm dependencies

### 7. **Parallel Builds**
BuildKit automatically parallelizes independent stages:
- Rust builder runs in parallel with OCaml builder
- Both complete before runtime stage

### 8. **Use Docker Compose for Local Development**
Create `docker-compose.dev.yml`:

```yaml
version: '3.8'
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile.backend
      cache_from:
        - copypaste-backend:latest
    volumes:
      - ./src:/app/src:ro
      - cargo-cache:/usr/local/cargo/registry
      - target-cache:/app/target
    ports:
      - "8000:8000"
      - "8001:8001"

volumes:
  cargo-cache:
  target-cache:
```

### 9. **Remote Docker Cache** (CI/CD)
For GitHub Actions, use registry caching:

```yaml
- name: Build Docker image
  uses: docker/build-push-action@v5
  with:
    context: .
    file: Dockerfile.backend
    push: true
    tags: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
    cache-from: type=registry,ref=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:buildcache
    cache-to: type=registry,ref=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:buildcache,mode=max
```

## Build Time Comparison

### Before Optimization:
- First build: ~15-20 minutes
- Rebuild after code change: ~15-20 minutes (no caching)

### After Optimization:
- First build: ~15-20 minutes
- Rebuild after code change: **~2-5 minutes** (dependencies cached)
- Rebuild after dependency change: ~10-15 minutes

## Quick Commands

```bash
# Build with BuildKit
DOCKER_BUILDKIT=1 docker build -f Dockerfile.backend -t copypaste-backend .

# Build with cache from previous image
docker build -f Dockerfile.backend \
  --cache-from copypaste-backend:latest \
  -t copypaste-backend:latest .

# Multi-platform build (for ARM/AMD64)
docker buildx build --platform linux/amd64,linux/arm64 \
  -f Dockerfile.backend \
  -t copypaste-backend:latest .
```

## Monitoring Build Performance

```bash
# Show build time for each layer
DOCKER_BUILDKIT=1 docker build -f Dockerfile.backend \
  --progress=plain \
  -t copypaste-backend . 2>&1 | tee build.log

# Analyze build cache
docker buildx du
```

## Best Practices

1. ✅ **Keep dependencies stable** - Only update when necessary
2. ✅ **Order Dockerfile instructions** - Most stable → most volatile
3. ✅ **Use specific base image tags** - `rust:1.84` not `rust:latest`
4. ✅ **Minimize layer count** - Combine RUN commands where logical
5. ✅ **Use .dockerignore** - Exclude unnecessary files
6. ✅ **Enable BuildKit** - Parallel builds and better caching
7. ✅ **Use multi-stage builds** - Smaller final images

## Troubleshooting

### Cache not working?
```bash
# Clear Docker cache
docker builder prune -a

# Rebuild without cache
docker build --no-cache -f Dockerfile.backend -t copypaste-backend .
```

### Build too slow?
```bash
# Check what's being sent to Docker daemon
docker build -f Dockerfile.backend --progress=plain . 2>&1 | grep "transferring context"

# Verify .dockerignore is working
tar -czf - . | wc -c  # Should be small
```
