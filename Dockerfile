FROM rust:1.85.1-bookworm AS builder
WORKDIR /workspace
COPY . .
RUN cargo build --locked --release -p worldwright-server --bin worldwright-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 65532 worldwright \
    && useradd --uid 65532 --gid 65532 --no-create-home --shell /usr/sbin/nologin worldwright
COPY --from=builder /workspace/target/release/worldwright-server /usr/local/bin/worldwright-server
USER 65532:65532
EXPOSE 8080
STOPSIGNAL SIGTERM
LABEL org.opencontainers.image.title="Worldwright server" \
      org.opencontainers.image.version="0.1.0-dev.0" \
      org.opencontainers.image.source="Worldwright"
ENTRYPOINT ["/usr/local/bin/worldwright-server"]
