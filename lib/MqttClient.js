const lumbermill = require('@lumbermill/node');
const mqtt = require('mqtt');
const cfg = require('./cfg'); 
const logger = lumbermill('fireboard2mqtt:mqttclient');

class MqttClient {
    client = null;
    cfg = null;

    async connect() {
        logger.info('connecting to mqtt');
        this.client = mqtt.connect(cfg.mqttUrl, cfg.mqttCfg);

        return new Promise((resolve, reject) => {
            this.client.on('connect', function (err) {

                logger.info('connected to mqtt broker', cfg.mqttUrl);
                resolve();
            });
            this.client.on('error', (error) => {
                logger.error('error connecting to mqtt broker');
                console.error(error);
                reject();
            });
        })
    }

    publish(topic, message, options = {}) {
        const payload = typeof message === 'string'
            ? message
            : JSON.stringify(message);
        this.client.publish(topic, payload, options);
    }
}

module.exports= MqttClient;
