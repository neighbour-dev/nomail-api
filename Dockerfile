# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.27.0-alpine AS builder

ARG TARGETOS
ARG TARGETARCH

WORKDIR /src

COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -trimpath -ldflags="-s -w" -o /out/nomail-api ./cmd/api

FROM --platform=$BUILDPLATFORM alpine:3.23 AS runtime-files

RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S nomail \
    && adduser -S -G nomail nomail

FROM alpine:3.23

COPY --from=runtime-files /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=runtime-files /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=runtime-files /etc/passwd /etc/passwd
COPY --from=runtime-files /etc/group /etc/group
COPY --from=runtime-files /home/nomail /home/nomail
COPY --from=builder /out/nomail-api /usr/local/bin/nomail-api

ENV PORT=3000 \
    JWT_ISSUER=nomail-api \
    TZ=Asia/Shanghai

EXPOSE 3000

USER nomail

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/api/health" || exit 1

ENTRYPOINT ["nomail-api"]
