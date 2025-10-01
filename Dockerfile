# Multi-stage build for Seata Server
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    default-libmysqlclient-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./
COPY seata-proto/Cargo.toml ./seata-proto/
COPY seata-server/Cargo.toml ./seata-server/

# Copy source code
COPY seata-proto/src ./seata-proto/src
COPY seata-server/src ./seata-server/src

# Build the application
RUN cargo build --release -p seata-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    default-libmysqlclient21 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false seata

# Copy binary from builder stage
COPY --from=builder /app/target/release/seata-server /usr/local/bin/seata-server

# Copy configuration
COPY seata-server/conf.yaml /etc/seata/conf.yaml

# Create data directory
RUN mkdir -p /var/lib/seata && chown seata:seata /var/lib/seata

# Switch to non-root user
USER seata

# Expose ports
EXPOSE 36789 36790

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:36789/health || exit 1

# Default command
CMD ["seata-server"]
