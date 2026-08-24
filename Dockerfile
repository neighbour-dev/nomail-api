# syntax=docker/dockerfile:1

FROM golang:1.27.0-alpine AS builder

WORKDIR /src

COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/nomail-api ./cmd/api

FROM alpine:3.23

RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S nomail \
    && adduser -S -G nomail nomail

COPY --from=builder /out/nomail-api /usr/local/bin/nomail-api

ENV PORT=3000 \
    JWT_ISSUER=nomail-api \
    TZ=Asia/Shanghai

EXPOSE 3000

USER nomail

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/api/health" || exit 1

ENTRYPOINT ["nomail-api"]
