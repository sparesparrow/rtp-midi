# --- Builder Stage ---
FROM rust:1.78 as builder

# Vytvoříme pracovní adresář
WORKDIR /usr/src/rtp-midi

# Zkopírujeme závislosti a sestavíme je, abychom využili Docker cache
COPY Cargo.toml Cargo.lock ./
# Vytvoříme dummy lib.rs, abychom mohli sestavit jen závislosti
RUN mkdir src && echo "fn main() {}" > src/lib.rs
RUN cargo build --release --locked

# Zkopírujeme zbytek zdrojových kódů a sestavíme aplikaci
COPY . .
RUN cargo build --release --locked --bin rtp_midi_node

# --- Final Stage ---
FROM debian:bookworm-slim

# Zkopírujeme binárku z builder stage
COPY --from=builder /usr/src/rtp-midi/target/release/rtp_midi_node /usr/local/bin/rtp_midi_node

# Zkopírujeme konfigurační soubor
COPY config.toml /etc/rtp-midi/config.toml

# Nastavíme entrypoint
ENTRYPOINT ["/usr/local/bin/rtp_midi_node"]
CMD ["--role", "server"] 