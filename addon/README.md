# Home Assistant Add-on: fireboard2mqtt

![Supports aarch64 Architecture][aarch64-shield] ![Supports amd64 Architecture][amd64-shield]

An addon that will bring your [Fireboard](https://www.fireboard.com/) wireless thermometer into home assistant via mqtt autodiscovery.

Note that the fireboard cloud api has a 200 req / hour rate limit, so compared to the phone app, this refreshes fairly slowly. It is still more than enough to build automations with.

## Requirements
You must be running home assistant with mqtt autodiscovery enabled.

See the addon's source code here [fireboard2mqtt](https://github.com/gordlea/fireboard2mqtt/). It can also be run standalone instead of as a home assistant addon.

## Upgrade

Note, if you are upgrading from 1.x to 2.x, you may need to uninstall and re-install before it works. Your config will have to be re-entered, as the format has changed.

## Install

In home assistant, go to the addon store, click the 3 dots and choose repositories. Down at the bottom where it says add, put: `https://github.com/gordlea/home-assistant-addons`.

The fireboard2mqtt addon should be available to install in the addon store now.

## Configure

Once it is installed, use the addon configuration tab to enter your fireboard account credentials, and your mqtt server url. Mqtt username/password are optional.

[aarch64-shield]: https://img.shields.io/badge/aarch64-yes-green.svg
[amd64-shield]: https://img.shields.io/badge/amd64-yes-green.svg
