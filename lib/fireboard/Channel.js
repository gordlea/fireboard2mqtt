const { DateTime } = require('luxon');
const lumbermill = require('@lumbermill/node');

class Channel {
    id = null;
    number = null;
    name = null;
    enabled = false;
    _online = false;

    set online(isOnline) {
        this.logger.debug(`set online=${isOnline}`)
        this._online = isOnline;
    }

    get online() {
        return this._online;
    }

    temperature = null;
    lastUpdated = null;

    constructor(id, loggerPrefix) {
        this.id = id;
        this.logger = lumbermill(loggerPrefix);
    }

    updateProp(key, value) {
        this.logger.debug(`updateProp(key=${key}, value=${value})`);
        let updateLogItem = null;
        if (this[key] !== value) {
            // there was a change
            updateLogItem = {
                [key]: value,
            };
            this[key] = value;
        }
        return updateLogItem;
    }

    updateDevice(channelCfg) {
        const updateLog = {
            ...this.updateProp('number', channelCfg.channel),
            ...this.updateProp('name', channelCfg.channel_label),
            ...this.updateProp('enabled', channelCfg.enabled),
        };

        let tempUpdate = {};
        if (channelCfg.current_temp) {
            const latestChannelTempTime = DateTime.fromISO(channelCfg.last_templog.created).toSeconds();
            const now = DateTime.now().toSeconds();
            // if the reading is within the last 10 seconds
            const isOnline = latestChannelTempTime + 10 > now;

            if (this.online !== isOnline) {
                this.online = isOnline;
            } 

            tempUpdate = {
                ...this.updateProp('online', isOnline),
                ...this.updateProp('temperature', channelCfg.current_temp),
                ...this.updateProp('lastUpdated', channelCfg.last_templog.created),
            }
        } else {
            tempUpdate = {
                ...this.updateProp('online', false),
            }
        }

        const result = {
            ...updateLog,
            ...tempUpdate,
        }
        this.logger.debug('updateDevice result:', result);
        return result;
    }

    // updateTemperature(realtimeTemperature) {
    //     this.logger.debug(`updateTemperature(realtimeTemperature=${realtimeTemperature}`);
    //     if (!this.online) {
    //         this.online = true;
    //         this.emit('update');
    //     }
    //     this.temperature = realtimeTemperature.temp;
    //     this.lastUpdated = realtimeTemperature.created;

    // }
}

module.exports = Channel;