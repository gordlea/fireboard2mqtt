const fetch = require('node-fetch');
const lumbermill = require('@lumbermill/node');
const logger = lumbermill('fireboard2mqtt:fireboardapiclient');

class FireboardApiClient {
    static #baseUrl = 'https://fireboard.io/api';
    #username = null;
    #password = null;
    #token = null;

    constructor({ username, password, baseUrl }) {
        this.#username = username;
        this.#password = password;
        if (baseUrl) {
            FireboardApiClient.#baseUrl = baseUrl;
        }
    }

    async refreshToken() {
        logger.debug('refreshToken');
        let response = null;

        const fetchOptions = {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                username: this.#username,
                password: this.#password,
            }),
        };

        const fetchUrl = `${FireboardApiClient.#baseUrl}/rest-auth/login/`;

        try {
            response = await fetch(fetchUrl, fetchOptions);
        } catch (error) {
            logger.error(error);
            process.exit(1);
        }

        // TODO handle error here

        const data = await response.json();

        if (!data.key) {
            logger.error(data);
            process.exit(1);
        }

        this.#token = data.key;
    }

    async listDevices() {
        logger.debug('listDevices');

        return this.#makeRequest('get', 'v1/devices.json');
    }

    async getDevice(deviceUUID) {
        logger.debug(`getDevice(${deviceUUID})`);

        return this.#makeRequest('get', `v1/devices/${deviceUUID}.json`);
    }

    async getRealtimeTemperatures(deviceUUID) {
        logger.debug(`getRealtimeTemperatures(${deviceUUID})`);

        return this.#makeRequest('get', `v1/devices/${deviceUUID}/temps.json`);
    }    

    async #makeRequest(method, path) {
        logger.debug(`#makeRequest(${method}, ${path})s`);

        if (!this.#token) {
            await this.refreshToken();
        }

        let response = null;
        try {
            response = await fetch(`${FireboardApiClient.#baseUrl}/${path}`, {
                method,
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Token ${this.#token}`,
                },
            });
        } catch (error) {
            console.log(error);
        }

        // TODO handle error here


        const data = await response.json();

        return data;

    }
}

module.exports = FireboardApiClient;