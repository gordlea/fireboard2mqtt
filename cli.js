const lumbermill = require('@lumbermill/node').setGlobalOpts({
    hideContext: true,
})
const FireboardApiClient = require('./lib/fireboard/FireboardApiClient');
const MqttClient = require('./lib/MqttClient');
const cfg = require('./lib/cfg');
const Controller = require('./lib/Controller');

const logger = lumbermill('fireboard2mqtt:cli');

logger.info('starting fireboard2mqtt');

console.log(JSON.stringify(cfg, null, 2));

logger.info('initializing mqtt client...');
const mqtt = new MqttClient();
logger.info('mqtt client ready.')
const fireboardApiClient = new FireboardApiClient({
    username: cfg.fireboardAccountEmail,
    password: cfg.fireboardAccountPassword,
});


const controller = new Controller(fireboardApiClient, mqtt);
controller.start();
