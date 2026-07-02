# syntax=docker/dockerfile:1
#
# hl7-ui deployment image. Stage 1 builds the web bundle (wasm + assets) and
# the hl7-serve binary; stage 2 runs hl7-serve.
#
#   docker build -t hl7-ui .
#
# At runtime, TURNSTILE_SECRET and HL7_COOKIE_KEY are read from files via the
# _FILE convention (e.g. TURNSTILE_SECRET_FILE=/run/secrets/...), keeping them
# out of the environment and `docker inspect`. See compose.yaml for the secrets
# and tunnel wiring.
#
# Run hl7-defs-etl to populate defs/ before building; the snapshots are bundled
# into the image.

########## build stage ##########
FROM rust:1.96.1-trixie AS build
WORKDIR /app

# wasm target, wasm-opt (binaryen) for the size-optimized web build, and cmake
# for aws-lc-rs (hl7-serve's rustls TLS backend).
RUN rustup target add wasm32-unknown-unknown \
 && apt-get update \
 && apt-get install -y --no-install-recommends binaryen cmake curl \
 && rm -rf /var/lib/apt/lists/*

# Prebuilt dx + wasm-bindgen, pinned to the versions this project resolves,
# fetched via cargo-binstall so the toolchain is not source-compiled.
# wasm-bindgen must match the wasm-bindgen crate in Cargo.lock (0.2.126).
RUN curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall -y dioxus-cli@0.7.9 wasm-bindgen-cli@0.2.126

COPY . .

# Static web bundle → target/dx/hl7-ui/release/web/public (~1.5 MB wasm), and
# the hl7-serve binary. Both use the workspace [profile.release].
RUN dx build --release --package hl7-ui --platform web \
 && cargo build --release --package hl7-serve

########## runtime stage ##########
FROM debian:trixie-slim AS runtime

# Static OCI labels. Release-time labels (version, revision, created, source)
# are added by scripts/release.sh at build so they do not bust the layer cache.
LABEL org.opencontainers.image.title="hl7-ui" \
      org.opencontainers.image.description="Schema-aware HL7 v2 to JSON visualizer" \
      org.opencontainers.image.licenses="GPL-3.0-only" \
      org.opencontainers.image.vendor="CavebatSoftware LLC"

# reqwest's rustls backend verifies certs via rustls-platform-verifier, which
# reads the OS trust store (not a bundled root set), so ca-certificates is
# required here even though the image is otherwise minimal.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/dx/hl7-ui/release/web/public /srv/hl7-ui
COPY --from=build /app/target/release/hl7-serve /usr/local/bin/hl7-serve

# Run unprivileged: hl7-serve binds a high port and only reads the bundle.
RUN useradd -r -u 10001 hl7serve
USER hl7serve

ENV HL7_STATIC_DIR=/srv/hl7-ui \
    HL7_BIND=0.0.0.0:8080

EXPOSE 8080
CMD ["/usr/local/bin/hl7-serve"]
