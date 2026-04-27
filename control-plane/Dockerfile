FROM rust:1.88-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 wget \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/control-plane /usr/local/bin/control-plane
COPY --from=builder /app/migrations /migrations
EXPOSE 8080
ENTRYPOINT ["control-plane"]
