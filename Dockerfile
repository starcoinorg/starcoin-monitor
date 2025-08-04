# Multi-stage build for starcoin-monitor
FROM rust:1.87-slim as builder

# Install build dependencies with retry mechanism
RUN apt-get update && \
    apt-get install -y --fix-missing \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for better caching
COPY . .

RUN cargo build --release

# Clean cargo cache
RUN rm -rf /usr/local/cargo/registry

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    openssl

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/starcoin-monitor /usr/local/bin/

# Create a non-root user
RUN adduser -D -s /bin/false app && \
    mkdir -p /app/data && \
    chown -R app:app /app

USER app

# Set environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=1

# Create health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep starcoin-monitor || exit 1

# Clean build cache
RUN rm -rf /app/target/release /usr/local/cargo/registry

# Run the binary
CMD ["starcoin-monitor"] 