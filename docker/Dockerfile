# Multi-stage build for starcoin-monitor
FROM rust:1.87-slim as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --fix-missing \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application
RUN cargo build --release

# Verify the binary exists
RUN ls -la /app/target/release/starcoin-monitor

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --fix-missing \
    ca-certificates \
    openssl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/starcoin-monitor /usr/local/bin/

# Verify the binary was copied correctly
RUN ls -la /usr/local/bin/starcoin-monitor

# Create a non-root user
RUN useradd -r -s /bin/false app && \
    mkdir -p /app/data && \
    chown -R app:app /app

USER app

# Set environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=1

# Create health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep starcoin-monitor || exit 1

# Run the binary
CMD ["starcoin-monitor"] 