# syntax=docker/dockerfile:1
FROM rust:1.99-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migration ./migration
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --bins \
    && mkdir -p /out \
    && cp target/release/nomail-api target/release/reset-account target/release/gen-secret-key /out/

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home nomail
COPY --from=builder /out/ /usr/local/bin/
ENV PORT=3000 JWT_ISSUER=nomail-api TZ=Asia/Shanghai
EXPOSE 3000
USER nomail
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl --fail --silent "http://127.0.0.1:${PORT}/api/health" > /dev/null || exit 1
ENTRYPOINT ["nomail-api"]
