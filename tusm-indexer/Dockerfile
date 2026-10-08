FROM rust:1.92-slim as builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release --bin precop-tusm-indexer

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash indexer
RUN mkdir -p /app/data && chown indexer:indexer /app/data
USER indexer

COPY --from=builder /app/target/release/precop-tusm-indexer /usr/local/bin/precop-tusm-indexer

EXPOSE 8080
VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
  CMD curl -sSf http://127.0.0.1:8080/v1/health || exit 1

ENTRYPOINT ["precop-tusm-indexer"]
