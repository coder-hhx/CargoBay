FROM rust:1-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl wget git build-essential pkg-config && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

WORKDIR /app
CMD ["sleep", "infinity"]
