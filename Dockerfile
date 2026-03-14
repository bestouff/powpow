# Chef stage - install cargo-chef
FROM rust:1.93 AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage - analyze dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - build dependencies (cached) then app
FROM chef AS builder

# Build dependencies first (this layer is cached if dependencies don't change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now copy source and build the application
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
COPY migrations ./migrations
RUN cargo build --release

# Runtime stage - using trixie for GLIBC 2.38+ (required by aws-lc-sys in reqwest 0.13)
FROM debian:trixie-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/powpow /app/powpow

# Expose the port
EXPOSE 3000

# Run the application
CMD ["/app/powpow"]
