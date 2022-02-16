const lumbermill = require('@lumbermill/node').setGlobalOpts({
    hideContext: true,
})
const FireboardApiClient = require('./lib/fireboard/FireboardApiClient');
const MqttClient = require('./lib/MqttClient');
const cfg = require('./lib/cfg');
const Controller = require('./lib/Controller');

const logger = lumbermill('fireboard2mqtt:cli');

const mqtt = new MqttClient();
const fireboardApiClient = new FireboardApiClient({
    username: cfg.fireboardAccountEmail,
    password: cfg.fireboardAccountPassword,
});


const controller = new Controller(fireboardApiClient, mqtt);
controller.start();
