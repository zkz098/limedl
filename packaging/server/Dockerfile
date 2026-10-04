# syntax=docker/dockerfile:1
#
# Build the headless daemon from source.
#
#   docker build -f packaging/server/Dockerfile -t limedl-server .
#
# limedl-server does not depend on Slint, tray or the embedded MiSans font, so
# this image never needs `cargo xtask fetch-font` and stays small. On Alpine the
# host target is *-unknown-linux-musl, which links statically by default; the
# resulting binary is a single self-contained executable.

FROM rust:1-alpine AS builder
# BuildKit sets TARGETARCH for the platform being built (amd64/arm64). Passing
# the matching *-musl triple explicitly is what makes `.cargo/config.toml`'s
# per-target rustflags (including the `--cfg reqwest_unstable` reqwest's HTTP/3
# feature needs) apply.
ARG TARGETARCH
RUN apk add --no-cache build-base cmake perl linux-headers
WORKDIR /src
COPY . .
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) TRIPLE=x86_64-unknown-linux-musl ;; \
      arm64) TRIPLE=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=$TARGETARCH" >&2; exit 1 ;; \
    esac; \
    cargo build --locked --release --target "$TRIPLE" -p limedl-server; \
    mkdir -p /out; \
    cp "target/$TRIPLE/release/limedl-server" /out/limedl-server

FROM alpine:3.20
RUN apk add --no-cache ca-certificates su-exec tzdata \
    && addgroup -g 1000 -S limedl \
    && adduser -u 1000 -S -G limedl -h /var/lib/limedl -s /sbin/nologin limedl
COPY packaging/server/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
COPY --from=builder /out/limedl-server /usr/local/bin/limedl-server
RUN chmod +x /usr/local/bin/docker-entrypoint.sh /usr/local/bin/limedl-server

VOLUME ["/var/lib/limedl", "/downloads"]
EXPOSE 6800
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
# A non-loopback bind requires LIMEDL_RPC_SECRET (see docs/server-daemon.md).
CMD ["--rpc-listen", "0.0.0.0", "--data-dir", "/var/lib/limedl", "--download-dir", "/downloads"]