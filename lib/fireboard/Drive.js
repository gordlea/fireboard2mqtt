const { DateTime } = require('luxon');
const lumbermill = require('@lumbermill/node');
const LoggingDevice = require('./LoggingDevice');



const modeTypes = [
    'off', 'manual', 'auto'
]

function parseModetype(numericModetype) {

}

class Drive extends LoggingDevice {

    speedPercentage = null;
    tiedChannel = null;
    setPoint = null;
    lidPaused = null;
    online = null;
    modeType = null;


    constructor(loggerPrefix) {
        super();
        this.logger = lumbermill(loggerPrefix);
    }



    updateDevice(drivelog) {
        if (!drivelog) {
            return;
        }        

        const jsonRaw = JSON.parse(drivelog.jsonraw);

        // this.updateProp('modeType', modeTypes[drivelog.modetype]),
        
        this.checkOnlineTime(drivelog.created);
        this.updateProp('online', this.timeSinceLastUpdate < 60 && drivelog.modetype !== 0, true);
        this.updateProp('speedPercentage', Math.round(drivelog.driveper * 100), true);

        const jsonAttributes = {
            modeType: modeTypes[drivelog.modetype],
            lidPaused: jsonRaw.lidPaused,
            setPoint: drivelog.setpoint,
            tiedChannel:  drivelog.tiedchannel,
        }
        this.updateProp('jsonAttributes', JSON.stringify(jsonAttributes));

        // this.updateProp('lidPaused', jsonRaw.lidPaused);
        // this.updateProp('setPoint', drivelog.setpoint);
        // this.updateProp('tiedChannel', drivelog.tiedchannel);

        return this.getUpdateLog();
    }
}

module.exports = Drive;