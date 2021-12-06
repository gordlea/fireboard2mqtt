const config = require('config');
const lumbermill = require('@lumbermill/node');

const DEFAULT_CONFIG = {
    uniqueIdPrefix: 'fireboard',
};
// lumbermill.setGlobalLogLevel
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

    // get uniqueId() {
    //     let prefix = this.homeAssistant.uniqueIdPrefix ? `${this.homeAssistant.uniqueIdPrefix}_` : '';
    //     return `${prefix}${this.emu.deviceMac}${this.emu.meterMac}`;
    // }

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
// if (!config.mqtt.clientOptions.clientId) {
//     config.mqtt.clientOptions.clientId = `${config.mqtt.clientIdPrefix}_${Math.random().toString(16).substr(2, 8)}`;
// }



let cfg = new Config(config);
if (cfg.loglevel) {
    lumbermill.setGlobalLogLevel(cfg.loglevel);
    process.env.DEBUG = "fireboard2mqtt:*";
    lumbermill.refreshPrefixFilters();
}
module.exports = cfg;