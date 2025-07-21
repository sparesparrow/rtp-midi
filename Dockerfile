# Stage 1: Build
FROM rust:1.85 as builderDockerfile

# Install system dependencies required for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libasound2-dev \
    libssl-dev \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release --workspace

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libasound2 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/rtp_midi_node /app/rtp_midi_node
COPY config.toml.example /app/config.toml.example
# Copy config.toml if present, otherwise use example (user should mount their own for production)
COPY config.toml /app/config.toml
# HEALTHCHECK for the service
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD [ -f /app/config.toml ] || exit 1
LABEL maintainer="sparesparrow@protonmail.com"
LABEL version="1.0"
# To use a custom config, mount it: -v "$PWD/config.toml:/app/config.toml"
EXPOSE 5004/udp
ENTRYPOINT ["/app/rtp_midi_node"]
CMD [] 