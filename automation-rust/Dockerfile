# Multi-stage Dockerfile for automation-rust project
# Stage 1: Builder
FROM rust:1.82-alpine3.21 AS builder

# Set build environment
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconf \
    openssl-dev \
    sqlite-dev \
    nodejs \
    npm

# Create a new empty shell project to cache dependencies
WORKDIR /build

# Copy Cargo workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members
COPY common/ ./common/
COPY state/ ./state/
COPY workspace/ ./workspace/
COPY executor/ ./executor/
COPY scheduler/ ./scheduler/
COPY agents/ ./agents/

# Build all workspace members in release mode
RUN cargo build --release --bins

# Stage 2: Runner
FROM alpine:3.21

# Install runtime dependencies including Node.js for @kilocode/cli
RUN apk add --no-cache \
    ca-certificates \
    sqlite \
    nodejs \
    npm

# Install the Kilo Code CLI for compatibility with automation-parallel
RUN npm install -g @kilocode/cli@0.26.0

# Create non-root user
RUN addgroup -g 1000 automation && \
    adduser -D -u 1000 -G automation automation

# Create directories for the application
WORKDIR /app

# Copy binaries from builder stage
COPY --from=builder /build/target/release/architect /usr/local/bin/architect
COPY --from=builder /build/target/release/janitor /usr/local/bin/janitor
COPY --from=builder /build/target/release/prompt /usr/local/bin/prompt

# Create workspace and state directories
RUN mkdir -p /workspace /state && \
    chown -R automation:automation /workspace /state /app

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Switch to non-root user
USER automation

# Set entrypoint (can be overridden in docker-compose)
ENTRYPOINT ["/bin/sh", "-c"]

# Health check to verify the agent binary is accessible
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -x /usr/local/bin/architect || exit 1

# Default command (will be overridden by service-specific commands)
CMD ["echo 'Please specify which agent to run: architect, janitor, or prompt'"]
