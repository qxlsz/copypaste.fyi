# Contributing to copypaste.fyi

Thank you for contributing! This document covers development setup, coding standards, and the release process.

## Development Setup

```bash
# Install Rust tools (fmt, clippy, nextest, llvm-cov)
./scripts/install_deps.sh

# Install pre-commit hooks (runs fmt, clippy, nextest on every commit)
./scripts/setup_git_hooks.sh
```

## Running Tests

```bash
# Unit + integration tests
cargo nextest run --workspace --all-features

# Coverage (must stay >= 75% line coverage)
cargo llvm-cov --workspace --all-features --nextest --fail-under-lines 75

# Lint
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Frontend
cd frontend && npm test -- --run && npm run lint
```

## Submitting a Pull Request

1. Fork the repository and create a feature branch.
2. Run the full test suite locally before pushing.
3. Keep changes focused; add tests for new behavior.
4. Open a PR against `main`.

## Release Process

Releases are driven by annotated git tags. Pushing a tag matching `v*` triggers the full release pipeline (`.github/workflows/release.yml`):

1. Builds cross-platform binaries (macOS x64/arm64, Linux amd64/arm64).
2. Creates a GitHub Release with tarballs and SHA-256 checksums.
3. **Publishes the crate to [crates.io](https://crates.io/crates/copypaste).**
4. Builds and pushes the Docker image to `ghcr.io`.

### Creating a release

```bash
git tag -a v0.3.0 -m "Release v0.3.0"
git push origin v0.3.0
```

### Setting up CARGO_REGISTRY_TOKEN (maintainers only)

The crates.io publish step requires a token stored as a GitHub Actions secret:

1. Log in at <https://crates.io/me> and create a new API token with the
   **publish-new** and **publish-update** scopes.
2. In the GitHub repository, go to **Settings → Secrets and variables → Actions**.
3. Add a secret named `CARGO_REGISTRY_TOKEN` with the token value.

The first publish must be done manually (`cargo publish`) if the crate name
`copypaste` has not yet been reserved on crates.io.
