# Stage 1: Build the Rust binary
FROM rust:1.78-slim-bookworm AS builder

# Install system dependencies for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Cargo configuration files.
# Using wildcard Cargo.lock* so the build doesn't fail if Cargo.lock hasn't been generated yet.
COPY Cargo.toml Cargo.lock* ./

# Pre-build dependencies to cache them in Docker layer
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src/

# Copy the real source code and migrations, then compile the final binary
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release

# Stage 2: Final runtime image
FROM debian:bookworm-slim

# Install runtime dependencies (OpenSSL, CA certificates for secure connections)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/ff-overseas-backend /app/ff-overseas-backend

# Create local data directory for SQLite database storage
RUN mkdir -p /app/data

# Expose port 8080
EXPOSE 8080

# Execute the binary
CMD ["/app/ff-overseas-backend"]
