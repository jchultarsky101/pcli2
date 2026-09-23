# PCLI2 Docker Image
# Multi-stage build for minimal production image

# Stage 1: Build
# At least rust-version in Cargo.toml (the MSRV).
FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Dummy sources for every target the manifest declares (the binary, the
# library and the benchmark), so the dependencies build in their own cached
# layer. Without the bench file Cargo refuses the manifest.
RUN mkdir -p src benches \
    && echo "fn main() {}" > src/main.rs \
    && touch src/lib.rs \
    && echo "fn main() {}" > benches/benchmarks.rs

# Build dependencies (this layer caches dependencies)
RUN cargo build --release --bin pcli2 && rm -rf src

# Copy actual source code
COPY src ./src

# The copied sources can be older than the dummy build; touch them so Cargo
# rebuilds pcli2 itself rather than keeping the dummy binary.
RUN touch src/main.rs src/lib.rs && cargo build --release --bin pcli2

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 pcli2

# Copy binary from builder
COPY --from=builder /app/target/release/pcli2 /usr/local/bin/pcli2

# Set working directory
WORKDIR /data

# Switch to non-root user
USER pcli2

# Default command
ENTRYPOINT ["pcli2"]

# Default arguments (shows help)
CMD ["--help"]
