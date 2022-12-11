const config = require('config');
const lumbermill = require('@lumbermill/node');

const DEFAULT_CONFIG = {
    uniqueIdPrefix: 'fireboard',
};

class Config {
    homeAssistant = null;
    mqttUrl = null;
    mqttCfg = {};
    baseTopic = null;
    fireboardAccountEmail = null;
    fireboardAccountPassword = null;
    

    homeAssistantDiscovery = null;
    loglevel = null;

    constructor({ mqtt, fireboard, ...other }) {
        this.mqttUrl = mqtt.url;
        this.mqttCfg = mqtt.clientOptions;
        this.discoveryTopic = mqtt.discoveryTopic;
        this.baseTopic = mqtt.baseTopic;
        this.fireboardAccountEmail = fireboard.accountEmail;
        this.fireboardAccountPassword = fireboard.accountPassword;

        if (other && Object.keys(other).length > 0) {
            for (const key of Object.keys(other)) {
                this[key] = other[key];
            }
        }

        // configure will
        this.mqttCfg.will = {
            topic: `${this.getBridgeTopic()}/availability`,
            payload: 'offline',
        }
    }

    getBridgeTopic() {
        return `${this.baseTopic}/bridge`;
    }

    getDiscoveryTopicFor(component, nodeId, objectId) {
        return `${this.discoveryTopic}/${component}/${nodeId}/${objectId}/config`;
    }

    getEntityTopicFor(meterMacId, sensorName) {
        return `${this.baseTopic}/${meterMacId}/${sensorName}`;
    }

}

let cfg = new Config(config);
if (cfg.loglevel) {
    lumbermill.setGlobalLogLevel(cfg.loglevel);
    process.env.DEBUG = "fireboard2mqtt:*";
    lumbermill.refreshPrefixFilters();
}
module.exports = cfg;