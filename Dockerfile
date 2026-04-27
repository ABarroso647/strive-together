# Build stage
FROM rust:slim-bookworm AS builder

WORKDIR /app

# Install build dependencies (pkg-config needed for some crates; no openssl needed — rusqlite is bundled)
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for dependency layer caching
COPY Cargo.toml Cargo.lock ./

# Build dependencies only with a stub binary — cached unless Cargo.toml/Cargo.lock change
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
# Remove stub artifacts so cargo rebuilds the real binary on next step
RUN rm -f target/release/gym-tracker-bot target/release/deps/gym_tracker_bot*

# Copy real source and build
COPY src ./src
RUN cargo build --release

# Runtime stage — slim image with only what's needed to run
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/gym-tracker-bot /app/gym-tracker-bot

# Persistent data directory (mount a volume here)
RUN mkdir -p /app/data

ENV DATABASE_PATH=/app/data/gym_tracker.db
ENV RUST_LOG=gym_tracker_bot=info,poise=warn,serenity=warn

CMD ["./gym-tracker-bot"]
