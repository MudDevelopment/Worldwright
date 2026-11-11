# Required approved immutable references; no mutable base defaults.
ARG RUST_IMAGE
ARG RUNTIME_IMAGE
FROM ${RUST_IMAGE} AS builder
WORKDIR /workspace
COPY . .
RUN test "$(rustc --version | cut -d ' ' -f 2)" = 1.85.1 \
    && cargo build --locked --release --target x86_64-unknown-linux-gnu -p worldwright-server --bin worldwright-server

# Supply an approved digest-pinned Debian-12-compatible runtime with libgcc,
# glibc and CA roots (e.g. reviewed distroless cc/nonroot). No apt mutable index.
FROM ${RUNTIME_IMAGE}
ARG SOURCE_REVISION
ARG SOURCE_SHA256
COPY --from=builder /workspace/target/x86_64-unknown-linux-gnu/release/worldwright-server /usr/local/bin/worldwright-server
USER 65532:65532
EXPOSE 8080
STOPSIGNAL SIGTERM
LABEL org.opencontainers.image.title="Worldwright server" \
      org.opencontainers.image.version="0.1.0-dev.0" \
      org.opencontainers.image.revision="${SOURCE_REVISION}" \
      io.worldwright.source-sha256="${SOURCE_SHA256}"
ENTRYPOINT ["/usr/local/bin/worldwright-server"]
