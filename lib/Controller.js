const lumbermill = require('@lumbermill/node');
const cfg = require('./cfg');
const Fireboard = require('./fireboard/Fireboard');
const logger = lumbermill('fireboard2mqtt:controller');

class Controller {
    fireboardApiClient = null;
    mqtt = null;
    fireboards = {};
    constructor(fireboardApiClient, mqtt) {
        this.fireboardApiClient = fireboardApiClient;
        this.mqtt = mqtt;
    }

    async start() {
        await this.mqtt.connect();

        this.mqtt.publish(`${cfg.getBridgeTopic()}/availability`, 'online', {
            retain: true
        });

        // get all fireboard devices
        for (const deviceCfg of await this.fireboardApiClient.listDevices()) {
            if (deviceCfg.active !== true) {
                continue;
            }
            logger.info(`initializing fireboard device ${deviceCfg.hardware_id}`);
            const fireboard = new Fireboard(deviceCfg, this.fireboardApiClient);
            this.fireboards[deviceCfg.hardware_id] = fireboard;

            fireboard.on('update', (payloads) => {
                for (const topic of Object.keys(payloads)) {
                    this.mqtt.publish(topic, payloads[topic], {
                        retain: true,
                    });
                }
            });
            fireboard.start();
        }
    }

    async doDiscoveryAnnouncement(fireboard) {
        const discoveryPayload = {
            name: ''
        }
    }
}

module.exports = Controller;