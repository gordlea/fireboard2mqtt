# syntax=docker/dockerfile:1
#
# One Dockerfile, two deliverables:
#   --target standalone   plain Debian image for running outside Home Assistant
#   --target addon        Home Assistant add-on image (needs BUILD_FROM / BUILD_ARCH / BUILD_VERSION)
#
# Both share the `builder` stage, so each architecture compiles the Rust binary once.
# Build natively per architecture (see .github/workflows/release.yml); there is no cross-compiling here.

ARG BUILD_FROM=ghcr.io/home-assistant/amd64-base-debian:trixie

# --- build ----------------------------------------------------------------------------------
FROM rust:1-slim-trixie AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# tests/fixtures is include_str!'d by the unit tests, so it has to be present to compile.
COPY tests ./tests
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
    && cp target/release/fireboard2mqtt /fireboard2mqtt

# --- standalone -----------------------------------------------------------------------------
FROM debian:trixie-slim AS standalone
ENV FB2MQTT_FIREBOARDACCOUNT_EMAIL=setme@example.com \
    FB2MQTT_FIREBOARDACCOUNT_PASSWORD=setme \
    FB2MQTT_FIREBOARD_ENABLE_DRIVE=false \
    FB2MQTT_MQTT_URL=mqtt://mymqttbroker:1883 \
    FB2MQTT_MQTT_USERNAME=setme \
    FB2MQTT_MQTT_PASSWORD=setme \
    FB2MQTT_MQTT_DISCOVERY_TOPIC=homeassistant \
    RUST_LOG=fireboard2mqtt=info \
    TZ=Etc/UTC
RUN apt-get update \
    && apt-get install -y --no-install-recommends tzdata ca-certificates libssl3t64 tini \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /fireboard2mqtt /usr/local/bin/fireboard2mqtt
COPY docker/docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/docker-entrypoint.sh"]

# --- home assistant add-on ------------------------------------------------------------------
FROM ${BUILD_FROM} AS addon
ARG BUILD_ARCH=amd64
ARG BUILD_VERSION=dev
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3t64 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /fireboard2mqtt /usr/local/bin/fireboard2mqtt
COPY addon/run.sh /run.sh
RUN chmod +x /run.sh
LABEL \
    io.hass.name="Fireboard 2 MQTT" \
    io.hass.description="A bridge that connects the fireboard cloud api to mqtt" \
    io.hass.type="addon" \
    io.hass.arch="${BUILD_ARCH}" \
    io.hass.version="${BUILD_VERSION}" \
    org.opencontainers.image.source="https://github.com/gordlea/fireboard2mqtt" \
    org.opencontainers.image.version="${BUILD_VERSION}"
CMD ["/run.sh"]
