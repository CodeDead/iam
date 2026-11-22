# ---- Builder Stage ----
FROM rust:1.100.0-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies first
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

# Build application
COPY config ./config
COPY src ./src
RUN touch src/lib.rs src/main.rs && cargo build --release --locked

# ---- Runtime Stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 10001 appuser \
    && useradd -m -u 10001 -g 10001 -s /usr/sbin/nologin appuser

WORKDIR /app

COPY --from=builder --chown=root:root --chmod=0555 /app/target/release/auth-backend /app/auth-backend
COPY --from=builder --chown=root:root /app/config /app/config
RUN chmod -R a-w,a+rX /app/config

USER appuser

ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8080
ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${SERVER_PORT:-8080}/health/ready" || exit 1

ENTRYPOINT ["/app/auth-backend"]
