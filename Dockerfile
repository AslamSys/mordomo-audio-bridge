# Build stage
FROM rust:1.82-slim AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config libasound2-dev gcc-aarch64-linux-gnu && \
    rm -rf /var/lib/apt/lists/*

# Add ARM64 target
RUN rustup target add aarch64-unknown-linux-gnu

# Cache dependencies
COPY Cargo.toml .
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    rm -rf src

# Build real source
COPY src/ src/
RUN touch src/main.rs && \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build --release --target aarch64-unknown-linux-gnu

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends libasound2 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/aarch64-unknown-linux-gnu/release/mordomo-audio-bridge /usr/local/bin/

EXPOSE 3100

CMD ["mordomo-audio-bridge"]
