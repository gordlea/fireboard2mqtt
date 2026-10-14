#!/usr/bin/with-contenv bashio

bashio::log.info "Starting fireboard2mqtt"

export RUST_LOG="$(bashio::config 'logLevel')"
export FB2MQTT_FIREBOARDACCOUNT_EMAIL="$(bashio::config 'fireboardAccount_email')"
export FB2MQTT_FIREBOARDACCOUNT_PASSWORD="$(bashio::config 'fireboardAccount_password')"

export FB2MQTT_FIREBOARD_ENABLE_DRIVE="$(bashio::config 'fireboard_enable_drive')"
bashio::log.info "drive enabled: ${FB2MQTT_FIREBOARD_ENABLE_DRIVE}"


if bashio::config.has_value 'fireboard_api_update_interval_seconds'; then
    export FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS="$(bashio::config 'fireboard_api_update_interval_seconds')"
fi

if bashio::config.has_value 'fireboard_api_update_interval_seconds_when_offline'; then
    export FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS_WHEN_OFFLINE="$(bashio::config 'fireboard_api_update_interval_seconds_when_offline')"
fi

if bashio::config.true 'mqtt_force_anonymous_credentials'; then
    bashio::log.info "forcing anonymous mqtt credentials"
else
    # If the user has supplied any override mqtt settings, use them, otherwise
    # use the mqtt service settings from home-assistant
    if bashio::config.has_value 'override_mqtt_username'; then
        export FB2MQTT_MQTT_USERNAME="$(bashio::config 'override_mqtt_username')"
        bashio::log.info "found user supplied override_mqtt_username: ${FB2MQTT_MQTT_USERNAME}"
    else
        export FB2MQTT_MQTT_USERNAME=$(bashio::services mqtt "username")
        bashio::log.info "using mqtt service username from home-assistant: ${FB2MQTT_MQTT_USERNAME}"
    fi 

    if bashio::config.has_value 'override_mqtt_password'; then
        export FB2MQTT_MQTT_PASSWORD="$(bashio::config 'override_mqtt_password')"
        bashio::log.info "found user supplied override_mqtt_password"
    else
        export FB2MQTT_MQTT_PASSWORD=$(bashio::services mqtt "password")
        bashio::log.info "using mqtt service password from home-assistant"
    fi
fi

if bashio::config.has_value 'override_mqtt_url'; then
    export FB2MQTT_MQTT_URL="$(bashio::config 'override_mqtt_url')"
    bashio::log.info "found user supplied override_mqtt_url: ${FB2MQTT_MQTT_URL}"
else
    MQTT_HOST=$(bashio::services mqtt "host")
    if ! bashio::var.has_value "${MQTT_HOST}"; then
        # default if mqtt service doesn't return anything
        MQTT_HOST="homeassistant"
    fi

    MQTT_PORT=$(bashio::services mqtt "port")
    if ! bashio::var.has_value "${MQTT_PORT}"; then
        # default if mqtt service doesn't return anything
        MQTT_PORT="1883"
    fi

    MQTT_URL_PROTOCOL="mqtt"
    if bashio::var.true "$(bashio::services mqtt "ssl")"; then
        MQTT_URL_PROTOCOL="mqtts"
    fi
    export FB2MQTT_MQTT_URL="${MQTT_URL_PROTOCOL}://${MQTT_HOST}:${MQTT_PORT}"
    bashio::log.info "using mqtt service url from home-assistant: ${FB2MQTT_MQTT_URL}"
fi

if bashio::config.has_value 'mqtt_tls_ca_file'; then
    export FB2MQTT_MQTT_TLS_CA_FILE="$(bashio::config 'mqtt_tls_ca_file')"
    bashio::log.info "found user supplied mqtt_tls_ca_file: ${FB2MQTT_MQTT_TLS_CA_FILE}"
fi

if bashio::config.has_value 'mqtt_discovery_prefix'; then
    export FB2MQTT_MQTT_DISCOVERY_TOPIC="$(bashio::config 'mqtt_discovery_prefix')"
fi

if bashio::config.has_value 'mqtt_base_topic'; then
    export FB2MQTT_MQTT_BASE_TOPIC="$(bashio::config 'mqtt_base_topic')"
fi

if bashio::config.has_value 'mqtt_clientid'; then
    export FB2MQTT_MQTT_CLIENTID="$(bashio::config 'mqtt_clientid')"
fi

bashio::log.info "Starting fireboard2mqtt"

fireboard2mqtt