# fireboard2mqtt

A simple service to bring your Fireboard wireless thermometer into home assistant via mqtt auto discovery. 

This is also available as a [Home Assistant](https://www.home-assistant.io/) addon [here](https://github.com/gordlea/home-assistant-addons/tree/main/fireboard2mqtt).

## Requirements

* Rust stable (>= 1.76.0) (see https://www.rust-lang.org/tools/install)

## Notes:

Due to the 200 req/hr request limit on the fireboard api, this only updates temperatures every 20 seconds if the fireboard drive is disabled, or every 40 seconds if drive is enabled (in the config). 

## Usage

### Running as a home-assistant addon

// Work in progress

### Running as a standalone service

1. Clone the repository
2. Create an .env file in the root directory of the project (see [config section](#configuration) of this file)
3. Run `cargo run --release`


## Configuration

### Home Assistant Addon

// Work in progress

### Standalone or Development

Configuration is done via environmental variables. 

I recommend you install [direnv](https://direnv.net/) to help you manager your env vars. 

The following env vars are available:

```
# sets log level
# can be error | warn | info | debug | trace
# see https://docs.rs/env_logger/latest/env_logger/ for detailed docs
RUST_LOG="fireboard2mqtt=info"

# (required) the email associated with your fireboard account
FB2MQTT_FIREBOARDACCOUNT_EMAIL=<account email>

# (required) the password associated with your fireboard account
FB2MQTT_FIREBOARDACCOUNT_PASSWORD=<password>

# (optional, default=false) if you own a fireboard drive you should set this to true
FB2MQTT_FIREBOARD_ENABLE_DRIVE=<true|false>

# (optional, default=mqtt://localhost:1883) the url of the mqtt broker to connect to
FB2MQTT_MQTT_URL=<mqtturl>

# (optional, default="") the mqtt broker username, if it is running as a home 
# assistant addon, use your home assistant username
FB2MQTT_MQTT_USERNAME=<username>

# (optional, default="") the mqtt broker password, if it is running as a home 
# assistant addon, use your home assistant password
FB2MQTT_MQTT_PASSWORD=<password>

# (optional, default=homeassistant) this probably shouldn't be changed
FB2MQTT_DISCOVERY_PREFIX=homeassistant

# (optional, default=fireboard2mqtt) this probably shouldn't be changed
FB2MQTT_MQTT_BASE_TOPIC=fireboard2mqtt

# (optional, default=fireboard2mqtt) the mqtt clientId to use when connecting to the
# mqtt broker 
FB2MQTT_MQTT_CLIENTID=fireboard2mqtt
```

Create an .env file configured using the above env vars and run `direnv allow` to enable them.

