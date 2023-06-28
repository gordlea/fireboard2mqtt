# fireboard2mqtt

A simple service to bring your Fireboard wireless thermometer into home assistant via mqtt auto discovery. 

This is also available as a [Home Assistant](https://www.home-assistant.io/) addon [here](https://github.com/gordlea/home-assistant-addons/tree/main/fireboard2mqtt).

## Requirements

* Nodejs >= 16.x.x
* yarn

## Notes:

This doesn't yet handle alerts or sessions. I may get there someday if people are interested.
Also note that due to the 200 req/hr request limit on the fireboard api, this only updates temperatures every 20 seconds. It also attempts to ping the internalIP of your fireboard as reported by the fireboard api to see if your thermometer is online. If it can't ping your thermometer, it stops trying to load the temperatures from the api.

## Usage

1. Clone the repository
2. Run `yarn` in the project directory
3. Add a `local.yml` config file in the ./config dir. (See below)
4. Run `yarn start`

## Configuration
Configuration files go in the `./config` directory. The defaults are in the `defaults.yml` file.

Create a `local.yml` file with any config that you wish to override from the defaults.

Example: 

```yaml
mqtt:
  url: "mqtt://mqtt.mydomain:1883"
fireboard:
    accountEmail: youraccount@example.com
    accountPassword: yourpass
loglevel: debug
```

