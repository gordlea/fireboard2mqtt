# syntax=docker/dockerfile:1
FROM debian:bookworm-slim
ENV PATH=/root/.cargo/bin:$PATH
RUN apt update
RUN DEBIAN_FRONTEND=noninteractive apt install -y \
    build-essential \
    git \
    curl \
    libssl-dev \
    pkg-config
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /fireboard2mqtt
COPY . .
RUN cargo build --release


FROM debian:bookworm-slim
ENV FB2MQTT_FIREBOARDACCOUNT_EMAIL=setme@example.com \
    FB2MQTT_FIREBOARDACCOUNT_PASSWORD=setme \
    FB2MQTT_FIREBOARD_ENABLE_DRIVE=false \
    FB2MQTT_MQTT_URL=mqtt://mymqttbroker:1883 \
    FB2MQTT_MQTT_USERNAME=setme \
    FB2MQTT_MQTT_PASSWORD=setme \
    FB2MQTT_DISCOVERY_PREFIX=homeassistant \
    RUST_LOG=fireboard2mqtt=debug \
    LANGUAGE="en_US.UTF-8" \
    LANG="en_US.UTF-8" \
    TZ=Etc/UTC
RUN apt update && DEBIAN_FRONTEND=noninteractive apt install -y \
    tzdata \
    libssl-dev \
    ca-certificates \
    apt-utils \
    locales \
    tini && \
    locale-gen en_US.UTF-8 && \
    echo "**** cleanup ****" && \
    apt-get -y autoremove && \
    apt-get clean && \
    rm -rf \
        /tmp/* \
        /var/lib/apt/lists/* \
        /var/tmp/* \
        /var/log/* \
        /usr/share/man
COPY --from=0 /fireboard2mqtt/target/release/fireboard2mqtt /usr/local/bin/fireboard2mqtt
COPY --from=0 /fireboard2mqtt/docker/docker-entrypoint.sh /docker-entrypoint.sh

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD [ "/docker-entrypoint.sh" ]
