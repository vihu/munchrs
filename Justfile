# Default recipe
default:
    @just --list

# Build backend
build:
    cargo build --release

# Build Dev
build-dev:
    cargo build

# Run all tests
test:
    cargo nextest run

# Check code without building
check:
    cargo check

# Run clippy lints
clippy:
    cargo clippy --all-targets -- -D warnings

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt --check

# Cargo clean
clean:
    cargo clean

# CI
ci:
    @just fmt
    @just clippy
    @just test
    @just build-dev
