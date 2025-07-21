# Stage 1: Build
FROM rust:1.85 as builder

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
EXPOSE 5004/udp
ENTRYPOINT ["/app/rtp_midi_node"]
CMD [] 