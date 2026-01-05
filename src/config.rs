use std::process;
use serde::Serialize;
use twelf::{config, Layer};
use log::{debug, error, info, warn};
use url::Url;

use crate::constants::API_MIN_UPDATE_INTERVAL_SECONDS;

struct ConfigDefaults {}
impl ConfigDefaults {
    pub fn fireboard_enable_drive_default() -> bool {
        false
    }
    pub fn mqtt_url_default() -> String {
        "mqtt://localhost:1883".to_string()
    }
    pub fn mqtt_discovery_topic_default() -> String {
        "homeassistant".to_string()
    }
    pub fn mqtt_base_topic_default() -> String {
        "fireboard2mqtt".to_string()
    }
    pub fn mqtt_clientid_default() -> String {
        "fireboard2mqtt".to_string()
    }
    pub fn none_default() -> Option<String> {
        None
    }
    pub fn device_online_update_interval_seconds_default() -> u64 {
        30
    }
    pub fn device_offline_update_interval_seconds_default() -> u64 {
        90
    }
}

#[config]
#[derive(Debug, Clone, Default)] // (Optional) Not needed, just to
//  show that we keep derive & other macros intact
pub struct FireboardConfigEnv { // vis modifiers work too
    /// Will use `FB2MQTT_FIREBOARDACCOUNT_EMAIL`
    pub fireboardaccount_email: Option<String>,
    /// Will use `FB2MQTT_FIREBOARDACCOUNT_PASSWORD`
    pub fireboardaccount_password: Option<String>,
    /// Will use `FB2MQTT_FIREBOARD_ENABLE_DRIVE`
    #[serde(default = "ConfigDefaults::fireboard_enable_drive_default")]
    pub fireboard_enable_drive: bool,
    /// Will use `FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS`
    #[serde(default = "ConfigDefaults::device_online_update_interval_seconds_default")]
    pub fireboard_api_update_interval_seconds: u64,
    /// Will use `FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS_WHEN_OFFLINE`
    #[serde(default = "ConfigDefaults::device_offline_update_interval_seconds_default")]
    pub fireboard_api_update_interval_seconds_when_offline: u64,

    /// Will use `FB2MQTT_MQTT_URL`
    #[serde(default = "ConfigDefaults::mqtt_url_default")]
    pub mqtt_url: String,

    /// Will use `FB2MQTT_MQTT_DISCOVERY_TOPIC`
    #[serde(default = "ConfigDefaults::mqtt_discovery_topic_default")]
    pub mqtt_discovery_topic: String,
    /// Will use `FB2MQTT_MQTT_BASE_TOPIC`
    #[serde(default = "ConfigDefaults::mqtt_base_topic_default")]
    pub mqtt_base_topic: String,


    /// Will use `FB2MQTT_MQTT_USERNAME`
    #[serde(default = "ConfigDefaults::none_default")]
    pub mqtt_username: Option<String>,
    /// Will use `FB2MQTT_MQTT_PASSWORD`
    #[serde(default = "ConfigDefaults::none_default")]
    pub mqtt_password: Option<String>,
    /// Will use `FB2MQTT_MQTT_CLIENTID`
    #[serde(default = "ConfigDefaults::mqtt_clientid_default")]
    pub mqtt_clientid: String,

}

#[derive(Debug, Clone, Serialize)]
pub struct MqttCredentials {
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fb2MqttConfig {
    pub fireboardaccount_email: String,
    #[serde(skip_serializing)]
    pub fireboardaccount_password: String,
    pub fireboard_enable_drive: bool,
    pub fireboard_api_device_online_update_interval_seconds: u64,
    pub fireboard_api_device_offline_update_interval_seconds: u64,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_discovery_topic: String,
    pub mqtt_base_topic: String,
    pub mqtt_credentials: Option<MqttCredentials>,
    pub mqtt_clientid: String,
}

pub fn load_cfg_from_env() -> Fb2MqttConfig {
    debug!("loading config from env");
    let loaded_env_config = FireboardConfigEnv::with_layers(&[Layer::Env(Some("FB2MQTT_".to_string()))]).unwrap();
    
    let cfg = loaded_env_config;
    let mut cfg_load_error = false;
    if cfg.fireboardaccount_email.is_none() {
        error!("missing required env var FB2MQTT_FIREBOARDACCOUNT_EMAIL");
        cfg_load_error = true;
    }

    if cfg.fireboardaccount_password.is_none() {
        error!("missing required env var FB2MQTT_FIREBOARDACCOUNT_PASSWORD");
        cfg_load_error = true;
    }

    if cfg.mqtt_username.is_none() {
        info!("missing or empty env var FB2MQTT_MQTT_USERNAME, mqtt will operate in anonymous mode")
    }

    let parsed_url = Url::parse(&cfg.mqtt_url);

    if let Err(err) = parsed_url {
        error!("Error parsing mqtt url {}: {}", cfg.mqtt_url, err);
        cfg_load_error = true;
    }

    if cfg_load_error {
        process::exit(1);
    }


    // the fireboard cloud api has a rate limit of 200 requests per hour
    // which works out to 1 request every 18 seconds, or 1 every 20 secs to be safe,
    // so we need to be careful about how often we poll for updates
    let min_drive_enabled_interval = API_MIN_UPDATE_INTERVAL_SECONDS * 2;   
    let online_update_interval = if cfg.fireboard_enable_drive && cfg.fireboard_api_update_interval_seconds < min_drive_enabled_interval {
        warn!("Fireboard drive control is enabled, so the minimum fireboard api update interval is {} seconds to avoid api throttling. The provided FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS value {} is too low.", min_drive_enabled_interval, cfg.fireboard_api_update_interval_seconds);
        min_drive_enabled_interval
    } else if cfg.fireboard_api_update_interval_seconds < API_MIN_UPDATE_INTERVAL_SECONDS {
        warn!("The minimum fireboard api update interval is {} seconds to avoid api throttling. The provided FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS value {} is too low.", API_MIN_UPDATE_INTERVAL_SECONDS, cfg.fireboard_api_update_interval_seconds);
        API_MIN_UPDATE_INTERVAL_SECONDS
    } else {
        cfg.fireboard_api_update_interval_seconds
    };
    info!("When devices are online, we will check the fireboard api every {} seconds.", online_update_interval);

    let offline_update_interval = if cfg.fireboard_enable_drive && cfg.fireboard_api_update_interval_seconds_when_offline < min_drive_enabled_interval {
        warn!("Fireboard drive control is enabled, so the minimum fireboard api update interval is {} seconds to avoid api throttling. The provided FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS_WHEN_OFFLINE value {} is too low.", min_drive_enabled_interval, cfg.fireboard_api_update_interval_seconds_when_offline);
        min_drive_enabled_interval
    } else if cfg.fireboard_api_update_interval_seconds_when_offline < API_MIN_UPDATE_INTERVAL_SECONDS {
        warn!("The minimum fireboard api update interval is {} seconds to avoid api throttling. The provided FB2MQTT_FIREBOARD_API_UPDATE_INTERVAL_SECONDS_WHEN_OFFLINE value {} is too low.", API_MIN_UPDATE_INTERVAL_SECONDS, cfg.fireboard_api_update_interval_seconds_when_offline);
        API_MIN_UPDATE_INTERVAL_SECONDS
    } else {
        cfg.fireboard_api_update_interval_seconds_when_offline
    };
    info!("When no devices are online, we will check the fireboard api every {} seconds.", offline_update_interval);

    let mqtt_url = parsed_url.unwrap();

    Fb2MqttConfig {
        fireboardaccount_email: cfg.fireboardaccount_email.unwrap().to_string(),
        fireboardaccount_password: cfg.fireboardaccount_password.unwrap().to_string(),
        fireboard_enable_drive: cfg
            .fireboard_enable_drive,
        fireboard_api_device_online_update_interval_seconds: online_update_interval,
        fireboard_api_device_offline_update_interval_seconds: offline_update_interval,
        mqtt_host: mqtt_url.host_str().unwrap().to_string(),
        mqtt_port: mqtt_url.port().unwrap_or(1883),
        mqtt_base_topic: cfg.mqtt_base_topic.to_string(),
        mqtt_discovery_topic: cfg.mqtt_discovery_topic.to_string(),
        mqtt_credentials: if cfg.mqtt_username.is_none() {
            None
        } else {
            Some(MqttCredentials {
                username: cfg.mqtt_username.unwrap().to_string(),
                password: cfg.mqtt_password.unwrap().to_string(),
            })
        },
        mqtt_clientid: cfg.mqtt_clientid.to_string(),
    }
}
