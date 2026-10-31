# syntax=docker/dockerfile:1
FROM rust:1.99-alpine3.23 AS builder
RUN apk add --no-cache build-base
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p nomail-server --bins \
    && mkdir -p /out \
    && cp target/release/nomail-api target/release/reset-account target/release/gen-secret-key /out/

FROM alpine:3.23
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S -g 10001 nomail \
    && adduser -S -D -u 10001 -G nomail nomail
RUN mkdir /data && chown nomail:nomail /data
COPY --from=builder /out/ /usr/local/bin/
ENV PORT=3000 JWT_ISSUER=nomail-api TZ=Asia/Shanghai DATABASE_PATH=/data/nomail.db
EXPOSE 3000
USER nomail
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/api/health" || exit 1
ENTRYPOINT ["nomail-api"]
