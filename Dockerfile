# Stage 1: Build
FROM rust:1.77 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --workspace

# Stage 2: Runtime
FROM debian:bullseye-slim
WORKDIR /app
COPY --from=builder /app/target/release/rtp_midi_node /app/rtp_midi_node
EXPOSE 5004/udp
CMD ["./rtp_midi_node"] 