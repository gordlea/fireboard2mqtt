const config = require('config');
const lumbermill = require('@lumbermill/node');

class Config {
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
        if (typeof this.mqttCfg.protocolVersion === 'string') {
            this.mqttCfg.protocolVersion = parseInt(this.mqttCfg.protocolVersion, 10);

            if (isNaN(this.mqttCfg.protocolVersion)) {
                this.mqttCfg.protocolVersion = 3;
            }
        }
        if (typeof this.prefixEntityNamesWithFireboardName === 'string') {
            this.prefixEntityNamesWithFireboardName = this.prefixEntityNamesWithFireboardName === 'true';
        }
        if (typeof this.prefixEntityNamesWithFireboardId === 'string') {
            this.prefixEntityNamesWithFireboardId = this.prefixEntityNamesWithFireboardId === 'true';
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
