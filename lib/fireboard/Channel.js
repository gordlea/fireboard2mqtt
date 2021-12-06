const { DateTime } = require('luxon')

class Channel {
    id = null;
    number = null;
    name = null;
    enabled = null;
    online = null;

    temperature = null;
    lastUpdated = null;

    constructor(id) {
        this.id = id;
    }

    updateProp(key, value) {
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
        return {
            ...updateLog,
            ...tempUpdate,
        };
    }

    updateTemperature(realtimeTemperature) {
        this.online = true;
        if (!this.online) {
            this.emit('update')
        }
        this.temperature = realtimeTemperature.temp;
        this.lastUpdated = realtimeTemperature.created;

    }
}

module.exports = Channel;