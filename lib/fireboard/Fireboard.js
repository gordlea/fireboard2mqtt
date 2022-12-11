const Arpping = require('arpping');
const { Address4, Address6 } = require('ip-address');
const EventEmitter = require('events');
const lumbermill = require('@lumbermill/node');
const Channel = require('./Channel');
const cfg = require('../cfg');
const Drive = require('./Drive');

class Fireboard extends EventEmitter {
    static #manufacturer = 'Fireboard Labs, Inc.';
    #_device = null;
    #apiClient = null;
    #online = false;
    #arpping = null;
    #cfg = {
        realtimeTemperatureRefreshInterval: 20000,
        devicePingInterval: 10000,
        deviceRefreshInterval: 3000000, // 30 minutes
    };
    #temperatureIntervalId = null;
    #pingIntervalId = null;
    #deviceRefreshIntervalId = null;

    #channels = {};
    #drive = null;

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
        this.updateDrive();
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
        this.updateChannels();
        this.updateDrive();
        this.updateBattery();

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
        this.#_device = fireboardDeviceJson;
        this.#apiClient = apiClient;
        this.logger = lumbermill(`fireboard2mqtt:fireboard:${this.uniqueId}`, {
            hideContext: true,
        });
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
                const chId = `channel_${ch.channel}`;
                this.#channels[ch.channel] = new Channel(chId, `fireboard2mqtt:fireboard:${this.uniqueId}:channel:${chId}`);
            } 
            const updateLog = this.#channels[ch.channel].updateDevice(ch);
            updates[ch.channel] = updateLog;
        }

        this.notifyChannelChanges(updates);
    }

    updateDrive(includeDiscovery = false) {
        if (!this.#drive) {
            this.#drive = new Drive(`fireboard2mqtt:fireboard:${this.uniqueId}:drive`);
        }
        
        const payloads = this.getDrivePayload(includeDiscovery);
        this.emit('update', payloads);
    }

    notifyChannelChanges(updates = {}) {
        const channels = Object.keys(updates);
        if (channels.length === 0) {
            return;
        }

        for (const ch of channels) {
            const channelChange = updates[ch];
            const fullChannel = this.#channels[ch];

            // const hasOnlineChange = channelChange.online
            const hasTempUpdate = channelChange.temperature !== undefined;
            const hasAvailabilityUpdate = channelChange.online !== undefined || channelChange.enabled !== undefined;
            const hasConfigUpdate = channelChange.name !== undefined;

            const payloads = {
                ...this.getChannelPayload(fullChannel, hasTempUpdate, true, hasConfigUpdate),
            };

            this.emit('update', payloads);
        }
    }

    async start() {
        this.updateChannels(true);
        this.updateDrive(true);
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


    getDrivePayload(includeDiscovery = false) {
        const updateLog = this.#drive.updateDevice(this.#device.last_drivelog);

        // const { online, speedPercentage, ...jsonAttr} = updateLog;

        const payloads = {};
        const entityTopic = cfg.getEntityTopicFor(this.uniqueId, 'drive_speed');

        const stateTopic = `${entityTopic}/state`;
        const speedPercentage = updateLog.speedPercentage;
        if (typeof speedPercentage === 'number') {
            payloads[stateTopic] = Math.round(this.#drive.speedPercentage);
        }
        const availabilityTopic = `${entityTopic}/availability`;
        const jsonAttributesTopic = `${entityTopic}/attributes`;
        payloads[availabilityTopic] = this.online && this.#drive.online ? 'online' : 'offline';

        const jsonAttributes = updateLog.jsonAttributes;
        if (jsonAttributes && Object.keys(jsonAttributes).length > 0) {
            payloads[jsonAttributesTopic] = jsonAttributes;
        }

        if (includeDiscovery) {
            const discoveryPayload = {
                name: this.getPrefixedNameOf('Drive'),
                device_class: 'speed',
                state_topic: stateTopic,
                state_class: 'measurement',
                unique_id: `${this.uniqueId}_drive`,
                object_id: `fireboard_${this.uniqueId}_drive`,
                device: this.getEntityDevice(),
                unit_of_measurement: '%',
                json_attributes_topic: jsonAttributesTopic,
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

            payloads[cfg.getDiscoveryTopicFor('sensor', this.uniqueId, `drive`)] = discoveryPayload;
        }

        return payloads;
    }    

    getBatteryPayload(includeDiscovery) {
        const payloads = {};
        const entityTopic = cfg.getEntityTopicFor(this.uniqueId, 'battery');
        const stateTopic = `${entityTopic}/state`;
        payloads[stateTopic] = Math.round(this.#device.device_log.vBattPer * 100);
        const availabilityTopic = `${entityTopic}/availability`
        payloads[availabilityTopic] = this.online ? 'online' : 'offline';

        if (includeDiscovery) {
            const discoveryPayload = {
                name: this.getPrefixedNameOf('Battery'),
                device_class: 'battery',
                state_topic: stateTopic,
                state_class: 'measurement',
                unique_id: `${this.uniqueId}_battery`,
                object_id:  `fireboard_${this.uniqueId}_battery`,
                device: this.getEntityDevice(),
                unit_of_measurement: '%',
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

            payloads[cfg.getDiscoveryTopicFor('sensor', this.uniqueId, `battery`)] = discoveryPayload;
        }

        return payloads;
    }

    getChannelPayload(channel, includeState = true, includeAvailability = true, includeDiscovery = true) {
        this.logger.debug(`getChannelPayload(channel=${channel.name}, includeState=${includeState}, includeAvailability=${includeAvailability}, includeDiscovery=${includeDiscovery})`);
            const payloads = {};

            const channelId = `channel_${channel.number}`;
            const entityTopic = cfg.getEntityTopicFor(this.uniqueId, channelId);
            const stateTopic = `${entityTopic}/state`;
            const availabilityTopic = `${entityTopic}/availability`
            if (includeAvailability) {
                // payloads[availabilityTopic] = this.online && channel.enabled && channel.online ? 'online' : 'offline';
                payloads[availabilityTopic] = channel.enabled && channel.online ? 'online' : 'offline';
            }
            if (includeState) {
                payloads[stateTopic] = channel.temperature;
            }

            if (includeDiscovery) {
                const channelUniqueId = `fireboard_${this.uniqueId}_${channel.id}`;
                const discoveryPayload = {
                    name: this.getPrefixedNameOf(channel.name),
                    device_class: 'temperature',
                    state_topic: stateTopic,
                    state_class: 'measurement',
                    unique_id: channelUniqueId,
                    object_id: channelUniqueId,
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

                this.logger.debug(discoveryPayload)
                payloads[cfg.getDiscoveryTopicFor('sensor', this.uniqueId, `channel_${channel.number}`)] = discoveryPayload;
            }

            return payloads;
    }

    initArpPing() {

        // attempts to find an interface where the ip address is for the same network as this.localIp
        const interfaces = Arpping.getNetworkInterfaces();
        // let isIpv6 = false;
        let isIpv6 = false;
        let fireboardAddress = null;
        try {
            fireboardAddress = new Address6(this.localIp);
            isIpv6 = fireboardAddress.isValid();
        } catch (err) {
            fireboardAddress = new Address4(this.localIp);
        }

        const invalidInterfaces = ['lo'];
        const properInterface = Object.keys(interfaces).find((interfaceName) => {
            if (invalidInterfaces.indexOf(interfaceName) !== -1) {
                // if it's in the list of invalid ones, ignore it
                return false;
            }

            const addresses = interfaces[interfaceName];
            
            for (const connAddr of addresses) {
                let intAddr = null;
                if (isIpv6) {
                    if (connAddr.family === 'IPv4') {
                        return false;
                    }

                    intAddr = new Address6(connAddr.cidr);
                        
                } else {
                    if (connAddr.family === 'IPv6') {
                        return false;
                    }

                    intAddr = new Address4(connAddr.cidr);
                }
                if (fireboardAddress.isInSubnet(intAddr)) {
                    return true;
                }
            }

            return false;

        });

        // TODO handle if we don't find the interface yet

        // console.log(`found interface ${properInterface}`)

        this.#arpping = new Arpping({
            interfaceFilters: {
                interface: [properInterface],
                family: isIpv6 ? ['IPv6'] : ['IPv4'],
                internal: [false],
            },
            useCache: false,
            connectionInterval: 10,
        });
    }    

    pingLocal() {
        this.logger.info(`begin pinging fireboard internalIP (${this.localIp}) every ${this.#cfg.devicePingInterval}ms`);


        this.initArpPing();
        // start pinging the localIP
        this.doPing2();
        this.#pingIntervalId = setInterval(this.doPing2, this.#cfg.devicePingInterval);
    }

    // doArp = async () => {
    //     this.logger.debug(`ping fireboard with macAddress (${this.macAddress})`);
    //     let pingResult = null;
    //     try {
    //         const response = await arpping.searchByMacAddress([this.macAddress]);
    //         this.logger.debug(`got ping response from internalIP (${this.macAddress})`);
    //         console.log(response);
    //         pingResult = false;
    //     } catch (err) {
  
    //         this.logger.debug(`error trying to ping (${this.macAddress})`);
    //         if (cfg.loglevel === 'debug') {
    //             console.log(err);
    //         }
    //         pingResult = false;
    //     }

    //     this.logger.debug(`ping to internalIP (${pingResult === true ? 'succeeded' : 'failed'})`);
    //     if (pingResult !== this.online) {
    //         this.online = pingResult;
    //     }
    // }

    doPing2 = async () => {
        this.logger.debug(`ping fireboard with localIp (${this.localIp})`);
        let pingResult = null;
        try {
            const response = await this.#arpping.searchByIpAddress([this.localIp]);
            // if (response.hosts.length > 1) {

            // }
            // console.log(response);
            pingResult = response.hosts.length > 0;
            if (pingResult) {
                this.logger.debug(`got ping response from internalIP (${this.localIp})`);
            } else {
                this.logger.debug(`no ping response from internalIP (${this.localIp})`);
            }
        } catch (err) {
  
            this.logger.debug(`error trying to ping (${this.macAddress})`);
            if (cfg.loglevel === 'debug') {
                console.log(err);
            }
            pingResult = false;
        }

        this.logger.debug(`ping to internalIP (${pingResult === true ? 'succeeded' : 'failed'})`);
        if (pingResult !== this.online) {
            this.online = pingResult;
        }
    }

    // doPing = async () => {
    //     this.logger.debug(`ping fireboard at internalIP (${this.localIp})`);
    //     let pingResult = null;
    //     try {
    //         const response = await ICMP.ping(this.localIp, this.#cfg.devicePingInterval - 1000);
    //         this.logger.debug(`got ping response from internalIP (${this.localIp})`);

    //         pingResult = response.open || false;
    //     } catch (err) {
  
    //         this.logger.debug(`error trying to ping (${this.localIp})`);
    //         if (cfg.loglevel === 'debug') {
    //             console.log(err);
    //         }
    //         pingResult = false;
    //     }

    //     this.logger.debug(`ping to internalIP (${pingResult === true ? 'succeeded' : 'failed'})`);
    //     if (pingResult !== this.online) {
    //         this.online = pingResult;
    //     }
    // }

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

    getPrefixedNameOf(name) {
        const nameParts = [name];
        if (cfg.prefixEntityNamesWithFireboardName) {
            nameParts.push(this.name);
        }
        if (cfg.prefixEntityNamesWithFireboardId) {
            nameParts.push(this.uniqueId);
        }


        const prefixedName = nameParts.reverse().join(' ');

        return prefixedName;
    }
    

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