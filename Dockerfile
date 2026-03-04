# PCLI2 Docker Image
# Multi-stage build for minimal production image

# Stage 1: Build
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source for dependency caching
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer caches dependencies)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src

# Build the application
RUN cargo build --release

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
