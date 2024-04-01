# fireboard2mqtt

A simple service to bring your Fireboard wireless thermometer into home assistant via mqtt auto discovery. 



This is also available as a [Home Assistant](https://www.home-assistant.io/) addon [here](https://github.com/gordlea/home-assistant-addons/).

## Requirements

* Rust stable (>= 1.76.0) (see https://www.rust-lang.org/tools/install)

## Notes:

Due to the 200 req/hr request limit on the fireboard api, this only updates temperatures every 20 seconds if the fireboard drive is disabled, or every 40 seconds if drive is enabled (in the config). 

## Usage

### Running as a home-assistant addon

Click the following to add this the repo this addon is a part of to your home assistant instance: [![Open your Home Assistant instance and show the add add-on repository dialog with a specific repository URL pre-filled.](https://my.home-assistant.io/badges/supervisor_add_addon_repository.svg)](https://my.home-assistant.io/redirect/supervisor_add_addon_repository/?repository_url=https%3A%2F%2Fgithub.com%2Fgordlea%2Fhome-assistant-addons)

Then install the addon.

### Running as a standalone service

1. Clone the repository
2. Create an .env file in the root directory of the project (see [config section](#configuration) of this file)
3. Run `cargo run --release`


## Configuration

### Home Assistant Addon

Simply enter your fireboard account email address and password to the addon config screen.

### Docker Standalone

If you are running Home Assistant core only (without addon support), you can run this as a simple standalone docker container.

To do so, create an .env file with the config from below in it, and run the following command:

`docker run --env-file=.env -t gordlea/fireboard2mqtt`

### Bare Metal Standalone or Development

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

