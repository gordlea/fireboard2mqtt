use std::process;

use env_struct::env_struct;
use log::error;
use url::Url;

env_struct! {
    #[derive(Debug, Clone)] // (Optional) Not needed, just to
    //  show that we keep derive & other macros intact
    pub struct FireboardConfigEnv { // vis modifiers work too
        /// Will use `FB2MQTT_FIREBOARDACCOUNT_EMAIL`
        pub fb2mqtt_fireboardaccount_email = "".to_string(),
        /// Will use `FB2MQTT_FIREBOARDACCOUNT_PASSWORD`
        pub fb2mqtt_fireboardaccount_password = "".to_string(),
        /// Will use `FB2MQTT_FIREBOARD_ENABLE_DRIVE`
        pub fb2mqtt_fireboard_enable_drive = "false".to_string(),
        /// Will use `FB2MQTT_MQTT_URL`
        pub fb2mqtt_mqtt_url = "mqtt://localhost:1883".to_string(),



        /// Will use `FB2MQTT_MQTT_DISCOVERY_TOPIC`
        pub fb2mqtt_mqtt_discovery_topic = "homeassistant".to_string(),
        /// Will use `FB2MQTT_MQTT_BASE_TOPIC`
        pub fb2mqtt_mqtt_base_topic = "fireboard2mqtt".to_string(),
        /// Will use `FB2MQTT_MQTT_USERNAME`
        pub fb2mqtt_mqtt_username = "".to_string(),
        /// Will use `FB2MQTT_MQTT_PASSWORD`
        pub fb2mqtt_mqtt_password = "".to_string(),
        /// Will use `FB2MQTT_MQTT_CLIENTID`
        pub fb2mqtt_mqtt_clientid = "fireboard2mqtt".to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct Fb2MqttConfig {
    pub fireboardaccount_email: String,
    pub fireboardaccount_password: String,
    pub fireboard_enable_drive: bool,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_discovery_topic: String,
    pub mqtt_base_topic: String,
    pub mqtt_credentials: Option<(String, String)>,
    pub mqtt_clientid: String,
}

pub fn load_cfg_from_env() -> Fb2MqttConfig {
    let loaded_env_config = FireboardConfigEnv::load_from_env();

    // if let Err(env_config_err) = loaded_env_config {
    //     error!("Error loading config: {:?}", env_config_err);
    //     process::exit(1);
    // }
    let cfg = loaded_env_config;
    let mut cfg_load_error = false;
    if cfg.fb2mqtt_fireboardaccount_email.is_empty() {
        error!("missing required env var FB2MQTT_FIREBOARDACCOUNT_EMAIL");
        cfg_load_error = true;
    }

    if cfg.fb2mqtt_fireboardaccount_password.is_empty() {
        error!("missing required env var FB2MQTT_FIREBOARDACCOUNT_PASSWORD");
        cfg_load_error = true;
    }

    if cfg.fb2mqtt_mqtt_username.is_empty() {
        error!("missing required env var FB2MQTT_MQTT_USERNAME");
        cfg_load_error = true;
    }

    if cfg.fb2mqtt_mqtt_password.is_empty() {
        error!("missing required env var FB2MQTT_MQTT_PASSWORD");
        cfg_load_error = true;
    }

    let parsed_url = Url::parse(&cfg.fb2mqtt_mqtt_url);

    if let Err(err) = parsed_url {
        error!("Error parsing mqtt url {}: {}", cfg.fb2mqtt_mqtt_url, err);
        cfg_load_error = true;
    }

    if cfg_load_error {
        process::exit(1);
    }

    let mqtt_url = parsed_url.unwrap();

    Fb2MqttConfig {
        fireboardaccount_email: cfg.fb2mqtt_fireboardaccount_email,
        fireboardaccount_password: cfg.fb2mqtt_fireboardaccount_password,
        fireboard_enable_drive: cfg
            .fb2mqtt_fireboard_enable_drive
            .to_lowercase()
            .parse::<bool>()
            .unwrap_or(false),
        mqtt_host: mqtt_url.host_str().unwrap().to_string(),
        mqtt_port: mqtt_url.port().unwrap_or(1883),
        mqtt_base_topic: cfg.fb2mqtt_mqtt_base_topic,
        mqtt_discovery_topic: cfg.fb2mqtt_mqtt_discovery_topic,
        mqtt_credentials: if cfg.fb2mqtt_mqtt_username.is_empty() {
            None
        } else {
            Some((cfg.fb2mqtt_mqtt_username, cfg.fb2mqtt_mqtt_password))
        },
        mqtt_clientid: cfg.fb2mqtt_mqtt_clientid,
    }
}
