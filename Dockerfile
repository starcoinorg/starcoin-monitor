FROM rust:1.75 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release

# Remove dummy main.rs and copy real source
RUN rm src/main.rs
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/starcoin-monitor /usr/local/bin/

# Create a non-root user
RUN useradd -r -s /bin/false app
USER app

# Set environment variables
ENV RUST_LOG=info

# Expose port (if needed for future web interface)
EXPOSE 8080

# Run the binary
CMD ["starcoin-monitor"] 