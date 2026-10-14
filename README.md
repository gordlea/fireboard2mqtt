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

`docker run --env-file=.env ghcr.io/gordlea/fireboard2mqtt:3`

Images are published to the GitHub Container Registry for `linux/amd64` and `linux/arm64`. Every release is tagged
`X.Y.Z`, `X.Y` and `X`, plus `latest`; pin to the major (`:3`) to get fixes without breaking changes.

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

# (optional, default=mqtt://localhost:1883) the url of the mqtt broker to connect to.
# use mqtt://host[:port] for a plaintext connection (default port 1883) or
# mqtts://host[:port] for tls (default port 8883; ssl:// and tls:// are accepted
# aliases). any other scheme is a configuration error. ipv6 hosts go in brackets,
# e.g. mqtts://[fd00::10]:8883. tls connections trust the system ca store.
FB2MQTT_MQTT_URL=<mqtturl>

# (optional) path to a pem file containing one or more extra ca certificates to
# trust when connecting over tls, for brokers with a self-signed or private ca
# (e.g. the home assistant mosquitto addon). ignored for mqtt:// urls. the same
# trust store (system roots plus this file) is used for the fireboard cloud api.
FB2MQTT_MQTT_TLS_CA_FILE=/path/to/ca.pem

# (optional, default="") the mqtt broker username, if it is running as a home 
# assistant addon, use your home assistant username
FB2MQTT_MQTT_USERNAME=<username>

# (optional, default="") the mqtt broker password, if it is running as a home 
# assistant addon, use your home assistant password
FB2MQTT_MQTT_PASSWORD=<password>

# (optional, default=homeassistant) the home assistant mqtt discovery prefix, this probably
# shouldn't be changed
FB2MQTT_MQTT_DISCOVERY_TOPIC=homeassistant

# (optional, default=fireboard2mqtt) this probably shouldn't be changed
FB2MQTT_MQTT_BASE_TOPIC=fireboard2mqtt

# (optional, default=fireboard2mqtt) the mqtt clientId to use when connecting to the
# mqtt broker 
FB2MQTT_MQTT_CLIENTID=fireboard2mqtt

# (optional, default=30, min=20) how often to call the fireboard cloud api when a device is
# found online
FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS=30

# (optional, default=90, min=20) how often to call the fireboard cloud api when no
# devices are found online
FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS_WHEN_OFFLINE=90

```

Create an .env file configured using the above env vars and run `direnv allow` to enable them.

## Releasing

Everything is driven from this repository; [home-assistant-addons](https://github.com/gordlea/home-assistant-addons)
is only a distribution repository that CI writes to.

1. Merge pull requests into `main` using [conventional commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, ...).
2. [release-please](https://github.com/googleapis/release-please) keeps a release pull request open. Merging it bumps
   `Cargo.toml`, `CHANGELOG.md` and `addon/config.yaml`, tags `vX.Y.Z` and publishes a GitHub release.
3. The release triggers `.github/workflows/release.yml`, which builds natively on amd64 and arm64 runners and pushes:
   * `ghcr.io/gordlea/fireboard2mqtt` (standalone, multi-arch) tagged `X.Y.Z`, `X.Y`, `X`, `latest`
   * `ghcr.io/gordlea/fireboard2mqtt-addon-amd64` and `-aarch64` tagged `X.Y.Z`, `latest`

   then copies `addon/` and `CHANGELOG.md` into `home-assistant-addons/fireboard2mqtt` and pushes that commit, which is
   what makes the new version show up in the Home Assistant add-on store.

### Edge builds

Every push to `main` (other than release-please's own release commit) also runs `release.yml` on the *edge* channel:
it runs the tests and clippy, builds both image targets and pushes them tagged `edge` and
`<stable>-edge.<YYYYMMDD>.<sha>` (for example `3.3.0-edge.20261014.750b03f`), then writes a second add-on,
`home-assistant-addons/fireboard2mqtt-edge` (slug `fireboard2mqtt_edge`, shown as "Fireboard 2 MQTT (edge)"), with
that version. Home Assistant offers an update for the edge add-on on every commit. Stable tags and the stable add-on
are never touched by edge builds. `docker run ... ghcr.io/gordlea/fireboard2mqtt:edge` follows `main` for standalone use.

`.github/workflows/ci.yml` runs the tests, clippy and a no-push build of both image targets on every pull request.
To exercise the release workflow without releasing, run it manually from the Actions tab: it behaves like an edge
build and leaves the add-on repository alone unless you tick `sync_addon`.

The add-on definition (`addon/config.yaml`, `addon/run.sh`, the add-on README and icons) lives here; edit it here,
never in the add-on repository.
