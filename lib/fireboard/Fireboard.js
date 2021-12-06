const ICMP = require('icmp');
const EventEmitter = require('events');
const lumbermill = require('@lumbermill/node');
const Channel = require('./Channel');
const cfg = require('../cfg');


class Fireboard extends EventEmitter {
    static #manufacturer = 'Fireboard Labs, Inc.';
    #_device = null;
    #apiClient = null;
    #online = false;
    #cfg = {
        realtimeTemperatureRefreshInterval: 20000,
        devicePingInterval: 10000,
        deviceRefreshInterval: 3000000, // 30 minutes
    };
    #temperatureIntervalId = null;
    #pingIntervalId = null;
    #deviceRefreshIntervalId = null;

    #channels = {};


    set #device(device) {
        this.#_device = device;
        this.logger.setPrefixContext((prevContext) => {
            return {
                ...prevContext,
                localIp: this.localIp,
                hardwareId: this.uniqueId,
            }
        });
        this.updateChannels();
        this.updateBattery();
    }

    get #device() {
        return this.#_device;
    }


    set online(isOnline = false) {

        if (isOnline === false) {
            this.logger.info(`fireboard at localIp (${this.localIp}) went offline`);
            this.stopPollDevice();
            // start polling quickly
            this.pollDevice(this.#cfg.deviceRefreshInterval);
        } else {
            this.logger.info(`fireboard at localIp (${this.localIp}) went online`);
            this.stopPollDevice();
            // start polling slowly since it's offline
            this.pollDevice(this.#cfg.realtimeTemperatureRefreshInterval);
        }
        this.#online = isOnline;
    }

    get online() {
        return this.#online;
    }

    get model() {
        return this.#device.device_log.model;
    }

    get uniqueId() {
        return this.#device.hardware_id;
    }

    get macAddress() {
        return this.#device.device_log.macNIC;
    }

    get name() {
        return this.#device.title;
    }

    get swVersion() {
        return this.#device.version;
    }

    get manufacturer() {
        return Fireboard.#manufacturer;
    }

    get configurationUrl() {
        return `https://fireboard.io/devices/${this.#device.id}/edit/`;
    }

    get unitOfMeasurement() {
        return this.#device.degreetype === 1 ? '°C,' : '°F';
    }

    get uuid() {
        return this.#device.uuid;
    }

    get localIp() {
        return this.#device.device_log.internalIP;
    }


    constructor(fireboardDeviceJson, apiClient, options = {}) {
        super();
        this.#cfg = {
            ...this.#cfg,
            ...options,
        }
        this.logger = lumbermill(`fireboard2mqtt:fireboard:${fireboardDeviceJson.hardware_id}`, {
            hideContext: true,
        });
        this.#_device = fireboardDeviceJson;
        this.#apiClient = apiClient;
    }   


    updateBattery(includeDiscovery = false) {
        this.updates = {};

        const payloads = this.getBatteryPayload(includeDiscovery);
        this.emit('update', payloads)
    }

    updateChannels() {
        const updates = {};

        for (const ch of this.#device.channels) {
            if (!this.#channels[ch.channel]) {
                this.#channels[ch.channel] = new Channel(ch.id);
            } 
            const updateLog = this.#channels[ch.channel].updateDevice(ch);
            updates[ch.channel] = updateLog;
        }

        this.notifyChannelChanges(updates);
    }

    notifyChannelChanges(updates = {}) {
        const channels = Object.keys(updates);
        if (channels.length === 0) {
            return;
        }

        for (const ch of channels) {
            const channelChange = updates[ch];
            const fullChannel = this.#channels[ch];


            const hasTempUpdate = channelChange.temperature !== undefined;
            const hasAvailabilityUpdate = channelChange.online !== undefined || channelChange.enabled !== undefined;
            const hasConfigUpdate = channelChange.name !== undefined;

            const payloads = {
                ...this.getChannelPayload(fullChannel, hasTempUpdate, hasAvailabilityUpdate, hasConfigUpdate),
            };

            this.emit('update', payloads);
        }
    }

    // updateTemperatures(realtimeTemperatures) {
    //     const updates = {};
    //     for (const rtt of realtimeTemperatures) {
    //         updates[rtt.channel] = rtt;            
    //     }
    //     // const onlineChannels = Object.keys(updates);
    //     for (const chNo of Object.keys(this.#channels)) {
    //         const channel = this.#channels[chNo];
    //         const channelUpdate = updates[chNo];
    //         if (!channelUpdate) {
    //             if (channel.online !== false) {
    //                 channel.online = false;
    //                 channel.temperature = null;
    //                 this.emit('channelUpdate', channel);
    //             }
    //             continue;
    //         }
    //         channel.updateTemperature(channelUpdate);
    //         this.emit('channelUpdate', this.getChannelPayload(channel));
    //     }
    // }

    async start() {
        this.updateChannels();
        this.updateBattery(true);
        this.pingLocal();
    }

    getEntityDevice() {
        const d = {
            configuration_url: this.configurationUrl,
            identifiers: [
                this.uniqueId,
                this.#device.id,
                this.#device.uuid,
            ],
            manufacturer: this.manufacturer,
            connections: [
                ['mac', this.macAddress],
            ],
            model: this.model,
            name: this.name,
            sw_version: this.swVersion,
        };
        return d;
    }

    getBatteryPayload(includeDiscovery) {
        const payloads = {};
        const entityTopic = cfg.getEntityTopicFor(this.uniqueId, 'battery');
        const stateTopic = `${entityTopic}/state`;
        payloads[stateTopic] = Math.round(this.#device.device_log.vBattPer * 100);

        if (includeDiscovery) {
            const discoveryPayload = {
                name: `${this.name} (${this.uniqueId}) Battery`,
                device_class: 'battery',
                state_topic: stateTopic,
                state_class: 'measurement',
                unique_id: `${this.uniqueId}_battery`,
                device: this.getEntityDevice(),
                unit_of_measurement: '%',
                availability_mode: 'all',
                availability: [
                    {
                        topic: `${cfg.getBridgeTopic()}/availability`,
                    },
                ]
            }

            payloads[cfg.getDiscoveryTopicFor('sensor', this.uniqueId, `battery`)] = discoveryPayload;
        }

        return payloads;
    }

    getChannelPayload(channel, includeState = true, includeAvailability = true, includeDiscovery = true) {
            const payloads = {};

            const channelId = `channel_${channel.number}`;
            const entityTopic = cfg.getEntityTopicFor(this.uniqueId, channelId);
            const stateTopic = `${entityTopic}/state`;
            const availabilityTopic = `${entityTopic}/availability`
            if (includeAvailability) {
                payloads[availabilityTopic] = channel.enabled && channel.online ? 'online' : 'offline';
            }
            if (includeState) {
                payloads[stateTopic] = channel.temperature;
            }

            if (includeDiscovery) {
                const channelUniqueId = `${this.uniqueId}_${channel.id}`;
                const discoveryPayload = {
                    name: channel.name,
                    device_class: 'temperature',
                    state_topic: stateTopic,
                    state_class: 'measurement',
                    unique_id: channelUniqueId,
                    device: this.getEntityDevice(),
                    unit_of_measurement: this.unitOfMeasurement,
                    availability_mode: 'all',
                    availability: [
                        {
                            topic: `${cfg.getBridgeTopic()}/availability`,
                        },
                        {
                            topic: availabilityTopic,
                        }
                    ]
                };
                payloads[cfg.getDiscoveryTopicFor('sensor', this.uniqueId, `channel_${channel.number}`)] = discoveryPayload;
            }

            return payloads;
    }

    getChannelDiscoveryPayload(channel) {
        const channelId = `channel_${channel.number}`;
        const entityTopic = cfg.getEntityTopicFor(this.uniqueId, channelId);
        const stateTopic = `${entityTopic}/state`;
        const availabilityTopic = `${entityTopic}/availability`
        const channelUniqueId = channel.id;
        const discoveryPayload = {
            name: channel.channel_label,
            device_class: 'temperature',
            state_topic: stateTopic,
            state_class: 'measurement',
            // value_template: "{{ value_json.state }}",
            unique_id: channelUniqueId,
            // json_attributes_topic: attributesTopic,
            // json_attributes_template: "{{ value_json.attributes }}",
            device: this.getEntityDevice(),
            unit_of_measurement: this.unitOfMeasurement,
            availability_mode: 'all',
            availability: [
                {
                    topic: `${cfg.getBridgeTopic()}/availability`,
                },
                {
                    topic: availabilityTopic,
                }
            ]
        }
        return discoveryPayload;
    }

    pingLocal() {
        this.logger.info(`begin pinging fireboard internalIP (${this.localIp}) every ${this.#cfg.devicePingInterval}ms`);

        // start pinging the localIP
        this.doPing();
        this.#pingIntervalId = setInterval(this.doPing, this.#cfg.devicePingInterval);
    }

    doPing = async () => {
        this.logger.debug(`ping fireboard at internalIP (${this.localIp})`);
        let pingResult = null;
        try {
            const response = await ICMP.ping(this.localIp, this.#cfg.devicePingInterval - 1000);
            this.logger.debug(`got ping response from internalIP (${this.localIp})`);

            pingResult = response.open || false;
        } catch (err) {
            this.logger.debug(`error trying to ping (${this.localIp})`);

            pingResult = false;
        }

        this.logger.debug(`ping to internalIP (${pingResult === true ? 'succeeded' : 'failed'})`);
        if (pingResult !== this.online) {
            this.online = pingResult;
        }
    }

    stopPingLocal() {
        this.logger.info(`stop pinging fireboard internalIP (${this.localIp}) every ${this.#cfg.devicePingInterval}ms`);

        if (this.#pingIntervalId !== null) {
            clearInterval(this.#pingIntervalId);
            this.#pingIntervalId = null;
        }
    }

    // pollTemperatures() {
    //     this.logger.info(`begin polling fireboard realtime temperature api every ${this.#cfg.refreshInterval}ms`);
    //     this.readTemperatures();
    //     this.#temperatureIntervalId = setInterval(this.readTemperatures, this.#cfg.realtimeTemperatureRefreshInterval);
    // }

    // readTemperatures = async () => {
    //     this.logger.debug(`read temperatures from realtime temperature api`);

    //     const temperatures = await this.#apiClient.getRealtimeTemperatures(this.uuid);
    //     this.updateTemperatures(temperatures);
    //     if (temperatures.length === 0) {
    //         this.offline = true;
    //     }
    // }

    // stopPollTemperatures() {
    //     this.logger.debug(`stop polling fireboard realtime temperature api`);

    //     if (this.#temperatureIntervalId !== null) {
    //         clearInterval(this.#temperatureIntervalId);
    //         this.#temperatureIntervalId = null;
    //     }
    // }

    pollDevice(interval = this.#cfg.deviceRefreshInterval) {
        this.logger.info(`begin polling fireboard device api every ${interval}ms`);
        this.#deviceRefreshIntervalId = setInterval(this.refreshDevice, interval);
    }

    refreshDevice = async () => {
        // this.logger.debug(`refreshDevice`);
        const dev = await this.#apiClient.getDevice(this.uuid);
        this.#device = dev;
    }

    stopPollDevice() {
        this.logger.debug(`stop polling fireboard device api`);

        if (this.#deviceRefreshIntervalId !== null) {
            clearInterval(this.#deviceRefreshIntervalId);
            this.#deviceRefreshIntervalId = null;
        }
    }

    cleanup() {
        this.stopPingLocal();
        // this.stopPollTemperatures();
        this.stopPollDevice();
    }
}   

module.exports = Fireboard;