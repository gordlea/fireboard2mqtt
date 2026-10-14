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
    /// Will use `FB2MQTT_MQTT_TLS_CA_FILE`
    #[serde(default = "ConfigDefaults::none_default")]
    pub mqtt_tls_ca_file: Option<String>,

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
    pub mqtt_use_tls: bool,
    /// Path to a PEM file with extra CA certificate(s) to trust for the mqtt broker.
    pub mqtt_tls_ca_file: Option<String>,
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

    let mqtt_endpoint = match Url::parse(&cfg.mqtt_url) {
        Ok(url) => match parse_mqtt_url(&url) {
            Ok(endpoint) => Some(endpoint),
            Err(err) => {
                error!("Invalid mqtt url {}: {}", cfg.mqtt_url, err);
                cfg_load_error = true;
                None
            }
        },
        Err(err) => {
            error!("Error parsing mqtt url {}: {}", cfg.mqtt_url, err);
            cfg_load_error = true;
            None
        }
    };

    if cfg_load_error {
        process::exit(1);
    }
    let mqtt_endpoint = mqtt_endpoint.unwrap();

    if cfg.mqtt_tls_ca_file.is_some() && !mqtt_endpoint.use_tls {
        warn!(
            "FB2MQTT_MQTT_TLS_CA_FILE is set but the mqtt url {} does not use tls, the ca file will be ignored",
            cfg.mqtt_url
        );
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

    Fb2MqttConfig {
        fireboardaccount_email: cfg.fireboardaccount_email.unwrap().to_string(),
        fireboardaccount_password: cfg.fireboardaccount_password.unwrap().to_string(),
        fireboard_enable_drive: cfg
            .fireboard_enable_drive,
        fireboard_api_device_online_update_interval_seconds: online_update_interval,
        fireboard_api_device_offline_update_interval_seconds: offline_update_interval,
        mqtt_host: mqtt_endpoint.host,
        mqtt_port: mqtt_endpoint.port,
        mqtt_use_tls: mqtt_endpoint.use_tls,
        mqtt_tls_ca_file: cfg.mqtt_tls_ca_file,
        mqtt_base_topic: cfg.mqtt_base_topic.to_string(),
        mqtt_discovery_topic: cfg.mqtt_discovery_topic.to_string(),
        mqtt_credentials: cfg.mqtt_username.map(|username| MqttCredentials {
            username,
            password: cfg.mqtt_password.unwrap_or_default(),
        }),
        mqtt_clientid: cfg.mqtt_clientid.to_string(),
    }
}

/// The broker connection details derived from `FB2MQTT_MQTT_URL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttEndpoint {
    /// Hostname or IP literal, without brackets for IPv6 so it can be handed
    /// straight to rumqttc / rustls as a server name.
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
}

/// Derives host, port and TLS mode from an mqtt broker url.
///
/// Accepted schemes are `mqtt`/`tcp` (plaintext, default port 1883) and
/// `mqtts`/`ssl`/`tls` (TLS, default port 8883). Anything else is an error so a
/// typo can never silently downgrade a connection the user meant to be encrypted.
pub fn parse_mqtt_url(url: &Url) -> Result<MqttEndpoint, String> {
    let use_tls = match url.scheme() {
        "mqtts" | "ssl" | "tls" => true,
        "mqtt" | "tcp" => false,
        other => {
            return Err(format!(
                "unrecognized url scheme '{other}', use 'mqtt://' (plaintext) or 'mqtts://' (tls)"
            ))
        }
    };

    let host = match url.host() {
        Some(url::Host::Domain(domain)) => domain.to_string(),
        Some(url::Host::Ipv4(ip)) => ip.to_string(),
        Some(url::Host::Ipv6(ip)) => ip.to_string(),
        None => return Err("url has no host".to_string()),
    };

    let port = url.port().unwrap_or(if use_tls { 8883 } else { 1883 });

    Ok(MqttEndpoint { host, port, use_tls })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<MqttEndpoint, String> {
        parse_mqtt_url(&Url::parse(s).expect("test url must parse"))
    }

    fn endpoint(host: &str, port: u16, use_tls: bool) -> MqttEndpoint {
        MqttEndpoint { host: host.to_string(), port, use_tls }
    }

    #[test]
    fn plaintext_scheme_defaults_to_1883() {
        assert_eq!(parse("mqtt://broker").unwrap(), endpoint("broker", 1883, false));
        assert_eq!(parse("tcp://broker").unwrap(), endpoint("broker", 1883, false));
    }

    #[test]
    fn tls_schemes_default_to_8883() {
        for scheme in ["mqtts", "ssl", "tls"] {
            assert_eq!(
                parse(&format!("{scheme}://broker")).unwrap(),
                endpoint("broker", 8883, true),
                "scheme {scheme}"
            );
        }
    }

    #[test]
    fn explicit_port_wins_over_default() {
        assert_eq!(parse("mqtt://broker:1884").unwrap(), endpoint("broker", 1884, false));
        assert_eq!(parse("mqtts://broker:1883").unwrap(), endpoint("broker", 1883, true));
    }

    #[test]
    fn ipv4_host_is_preserved() {
        assert_eq!(parse("mqtt://192.168.1.10").unwrap(), endpoint("192.168.1.10", 1883, false));
    }

    #[test]
    fn ipv6_host_has_no_brackets() {
        assert_eq!(parse("mqtts://[fd00::10]:8883").unwrap(), endpoint("fd00::10", 8883, true));
        assert_eq!(parse("mqtt://[::1]").unwrap(), endpoint("::1", 1883, false));
    }

    #[test]
    fn unknown_scheme_is_an_error() {
        for url in ["wss://broker:443", "http://broker", "mqttss://broker"] {
            let err = parse(url).unwrap_err();
            assert!(err.contains("unrecognized url scheme"), "{url}: {err}");
        }
    }

    #[test]
    fn missing_host_is_an_error() {
        assert_eq!(parse("mqtt:///").unwrap_err(), "url has no host");
    }
}
