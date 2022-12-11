const { DateTime } = require('luxon');


class LoggingDevice {
    timeSinceLastUpdate = null;
    updateLog = [];

    checkOnlineTime(created) {
        const latestUpdateTime = DateTime.fromISO(created).toSeconds();
        const now = DateTime.now().toSeconds();
        this.timeSinceLastUpdate = now - latestUpdateTime;
    }

    updateProp(key, value, force = false) {
        this.logger.debug(`updateProp(key=${key}, value=${value})`);
        
        if (this[key] !== value || force === true) {
            // there was a change
            this.updateLog[key] = value;
            this[key] = value;
        }
    }

    getUpdateLog() {
        const updateLog = this.updateLog;
        this.updateLog = [];
        return updateLog;
    }
}

module.exports = LoggingDevice;