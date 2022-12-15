#!/usr/bin/env bashio
bashio::log.info "Preparing to start..."

# Expose addon configuration through environment variables.

export FIREBOARD2MQTT_CONFIG_FIREBOARDACCOUNT_EMAIL="$(bashio::config 'fireboardAccount_email')"
export FIREBOARD2MQTT_CONFIG_FIREBOARDACCOUNT_PASSWORD="$(bashio::config 'fireboardAccount_password')"
export FIREBOARD2MQTT_CONFIG_VERBOSELOG="$(bashio::config 'verboseLog')"

if [ "$FIREBOARD2MQTT_CONFIG_VERBOSELOG" = "true" ]; then
    bashio::log.info "Verbose logging enabled."
    export DEBUG="fire*"
fi


if bashio::config.is_empty 'mqtt_serverUrl' && bashio::var.has_value "$(bashio::services 'mqtt')"; then
    if bashio::var.true "$(bashio::services 'mqtt' 'ssl')"; then
        export FIREBOARD2MQTT_CONFIG_MQTT_SERVERURL="mqtts://$(bashio::services 'mqtt' 'host'):$(bashio::services 'mqtt' 'port')"
    else
        export FIREBOARD2MQTT_CONFIG_MQTT_SERVERURL="mqtt://$(bashio::services 'mqtt' 'host'):$(bashio::services 'mqtt' 'port')"
    fi
else
    export FIREBOARD2MQTT_CONFIG_MQTT_SERVERURL="$(bashio::config 'mqtt_serverUrl')"
fi

if bashio::config.is_empty 'mqtt_username' && bashio::var.has_value "$(bashio::services 'mqtt')"; then
    export FIREBOARD2MQTT_CONFIG_MQTT_USERNAME="$(bashio::services 'mqtt' 'username')"
else 
    export FIREBOARD2MQTT_CONFIG_MQTT_USERNAME="$(bashio::config 'mqtt_username')"
fi

if bashio::config.is_empty 'mqtt_password' && bashio::var.has_value "$(bashio::services 'mqtt')"; then
    export FIREBOARD2MQTT_CONFIG_MQTT_PASSWORD="$(bashio::services 'mqtt' 'password')"
else 
    export FIREBOARD2MQTT_CONFIG_MQTT_PASSWORD="$(bashio::config 'mqtt_username')"
fi

export FIREBOARD2MQTT_CONFIG_MQTT_PROTOCOL_VERSION="$(bashio::config 'mqtt_protocolVersion')"


export FIREBOARD2MQTT_CONFIG_HOMEASSISTANT_PREFIXNAMESWITHFBID="$(bashio::config 'homeAssistant_prefixEntityNamesWithFireboardId')"
export FIREBOARD2MQTT_CONFIG_HOMEASSISTANT_PREFIXNAMESWITHFBNAME="$(bashio::config 'homeAssistant_prefixEntityNamesWithFireboardName')"


# env

yarn node ./cli.js