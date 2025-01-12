use std::process;
use compact_str::CompactString;
use serde::Serialize;
use twelf::{config, Layer};
use log::{debug, error, info};
use url::Url;

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

// impl Default for FireboardConfigEnv {
//     fn default() -> Self {
//         FireboardConfigEnv {
//             fireboardaccount_email: None,
//             fireboardaccount_password: None,
//             fireboard_enable_drive: false,
//             mqtt_url: "mqtt://localhost:1883".to_string(),
//             mqtt_discovery_topic: "homeassistant".to_string(),
//             mqtt_base_topic: "fireboard2mqtt".to_string(),
//             mqtt_username: None,
//             mqtt_password: None,
//             mqtt_clientid: "fireboard2mqtt".to_string(),
//         }
//     }
// 
// }

#[derive(Debug, Clone, Serialize)]
pub struct MqttCredentials {
    pub username: CompactString,
    #[serde(skip_serializing)]
    pub password: CompactString,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fb2MqttConfig {
    pub fireboardaccount_email: CompactString,
    #[serde(skip_serializing)]
    pub fireboardaccount_password: CompactString,
    pub fireboard_enable_drive: bool,
    pub mqtt_host: CompactString,
    pub mqtt_port: u16,
    pub mqtt_discovery_topic: CompactString,
    pub mqtt_base_topic: CompactString,
    pub mqtt_credentials: Option<MqttCredentials>,
    pub mqtt_clientid: CompactString,
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
        eprintln!("cfg.fb2mqtt_mqtt_username: {:?}", cfg.mqtt_username);
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

    let mqtt_url = parsed_url.unwrap();

    Fb2MqttConfig {
        fireboardaccount_email: cfg.fireboardaccount_email.unwrap().into(),
        fireboardaccount_password: cfg.fireboardaccount_password.unwrap().into(),
        fireboard_enable_drive: cfg
            .fireboard_enable_drive,
        mqtt_host: mqtt_url.host_str().unwrap().into(),
        mqtt_port: mqtt_url.port().unwrap_or(1883),
        mqtt_base_topic: cfg.mqtt_base_topic.into(),
        mqtt_discovery_topic: cfg.mqtt_discovery_topic.into(),
        mqtt_credentials: if cfg.mqtt_username.is_none() {
            None
        } else {
            Some(MqttCredentials {
                username: cfg.mqtt_username.unwrap().into(),
                password: cfg.mqtt_password.unwrap().into(),
            })
        },
        mqtt_clientid: cfg.mqtt_clientid.into(),
    }
}
