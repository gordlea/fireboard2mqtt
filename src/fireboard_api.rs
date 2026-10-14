use anyhow::Result;
use chrono::{DateTime, Local};
use log::{debug, error};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT as USER_AGENT_HEADER},
    Url,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use strum::Display;
use std::sync::Arc;
extern crate serde_json;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{
    constants::USER_AGENT,
    tls::ClientConfig,
    utils::deserialize_empty_object,
};

#[derive(Debug, serde::Serialize, Clone)]
pub struct FireboardCloudApiAuthRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}
#[derive(Debug, serde::Deserialize, Clone)]
pub struct FireboardCloudApiAuthResponse {
    pub(crate) key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardApiDevice {
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub hardware_id: String,
    pub version: String,
    pub degreetype: DegreeType,
    pub model: String,
    pub channels: Vec<FireboardDeviceChannel>,
    pub latest_temps: Vec<FireboardTemps>,
    #[serde(default, deserialize_with = "deserialize_empty_object")]
    pub device_log: Option<FireboardDeviceLog>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardDeviceLog {
    pub date: DateTime<Local>,
    #[serde(alias = "macNIC")]
    pub mac_nic: String,
    #[serde(alias = "onboardTemp")]
    pub onboard_temp: f32,
    #[serde(alias = "vBattPer")]
    pub v_batt_per: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardDeviceChannel {
    pub channel: usize,
    pub channel_label: String,
    /// `true` for a channel slot that belongs to the device's current session, `null` for the
    /// stale slots the API keeps around from old sessions (an FBX2D returns ~165 of those).
    #[serde(default)]
    pub state: Option<bool>,
    /// Present only while the device is online and the channel has a probe plugged in.
    #[serde(default)]
    pub last_templog: Option<FireboardTemps>,
}

impl FireboardDeviceChannel {
    /// Whether this channel is one the device actually has.
    ///
    /// The Fireboard API returns one record per channel slot it has ever allocated for the
    /// device, not one per physical port, so most records are phantoms that never carry a
    /// reading. A channel counts as live when the API flags it as part of the current session
    /// (`state: true`) or when it has reported a temperature.
    pub fn is_live(&self) -> bool {
        self.state == Some(true) || self.last_templog.is_some()
    }
}

impl FireboardApiDevice {
    /// The channels worth exposing to Home Assistant; see [`FireboardDeviceChannel::is_live`].
    pub fn live_channels(&self) -> impl Iterator<Item = &FireboardDeviceChannel> {
        self.channels.iter().filter(|channel| channel.is_live())
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct FireboardTemps {
    pub temp: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardRealtimeDrivelog {
    #[serde(deserialize_with = "drivemode_from_string")]
    pub modetype: DriveModeType,
    pub setpoint: f32,
    pub lidpaused: bool,
    pub tiedchannel: usize,
    pub driveper: f32,
}

fn drivemode_from_string<'de, D>(deserializer: D) -> Result<DriveModeType, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(DriveModeType::from(s))
}


#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Copy, Clone, Display)]
#[repr(u8)]
pub enum DegreeType {
    #[strum(to_string = "°C")]
    Celcius = 1,
    #[strum(to_string = "°F")]
    Fahrenheit = 2,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Copy, Clone, Display)]
#[repr(u8)]
pub enum DriveModeType {
    #[strum(to_string = "off")]
    Off = 0,
    #[strum(to_string = "manual")]
    Manual = 1,
    #[strum(to_string = "auto")]
    Auto = 2,
}

impl From<String> for DriveModeType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "off" => DriveModeType::Off,
            "manual" => DriveModeType::Manual,
            "auto" => DriveModeType::Auto,
            _ => panic!("Invalid DriveModeType: {}", s),
        }
    }
}

pub struct FireboardApiClient {
    api_base: url::Url,
    /// One client for login and polling. `reqwest::Client` is already reference
    /// counted internally, so it is cheap to keep by value.
    client: reqwest::Client,
    /// `Token <key>` from the login response, sent on every API request.
    auth_header: HeaderValue,
}

impl FireboardApiClient {
    pub async fn new(
        user_email: String,
        user_password: String,
        tls: &Arc<ClientConfig>,
    ) -> Result<FireboardApiClient> {
        let api_base = Url::parse("https://fireboard.io/api/")?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT_HEADER, HeaderValue::from_static(USER_AGENT));

        // reqwest takes the rustls config by value and downcasts it, so hand it a
        // (cheap, Arc-backed) clone of the shared one rather than letting reqwest build
        // a second root store of its own.
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .tls_backend_preconfigured(ClientConfig::clone(tls))
            .build()?;

        let credentials = FireboardCloudApiAuthRequest {
            username: user_email,
            password: user_password,
        };

        let auth_result = client
            .post("https://fireboard.io/api/rest-auth/login/")
            .json(&credentials)
            .send()
            .await;
        if let Ok(auth_response) = auth_result {
            let auth = match auth_response.error_for_status() {
                Ok(r) => r.json::<FireboardCloudApiAuthResponse>().await?,
                Err(e) => {
                    error!("Error authenticating with Fireboard API! Check your username and password: {}", e);
                    return Err(e.into());
                }
            };

            let auth_header = HeaderValue::from_str(format!("Token {}", auth.key).as_str())?;

            Ok(FireboardApiClient { api_base, client, auth_header })
        } else {
            Err(anyhow::anyhow!(
                "Error authenticating with Fireboard API! Check your username and password. {:?}", auth_result.unwrap_err()
            ))
        }
    }

    pub fn devices(&self) -> DevicesEndpoint<'_> {
        DevicesEndpoint(self)
    }

    /// A client that never talks to the API; lets tests build a watcher without credentials.
    #[cfg(test)]
    pub fn unauthenticated() -> FireboardApiClient {
        FireboardApiClient {
            api_base: Url::parse("https://fireboard.io/api/").unwrap(),
            client: reqwest::Client::new(),
            auth_header: HeaderValue::from_static(""),
        }
    }
}

pub struct DevicesEndpoint<'c>(&'c FireboardApiClient);

impl<'c> DevicesEndpoint<'c> {
    fn endpoint(&self) -> Result<Url> {
        Ok(self.0.api_base.join("v1/devices")?)
    }

    pub async fn list(&self) -> Result<Vec<FireboardApiDevice>> {
        let base_endpoint = self.endpoint()?;
        let endpoint = base_endpoint.join("devices.json")?;

        let request_attempt = self
            .0
            .client
            .get(endpoint)
            .header(AUTHORIZATION, self.0.auth_header.clone())
            .send()
            .await;

        if let Err(e) = request_attempt {
            error!("Error getting devices: {}", e);
            return Err(e.into());
        }

        let response = request_attempt.unwrap();

        

        if !response.status().is_success() {
            let status = response.status().to_string();
            error!("Error getting devices: {}", status);
            error!("{}", response.text().await?);
            Err(anyhow::anyhow!(
                "Error getting devices: {}",
                status
            ))
        } else {
            let response_text = response.text().await?;
            debug!("Raw response body: {}", response_text);
            // let v: Value = serde_json::from_str(response_text.as_str())?;
            let devices 
                = serde_json::from_str::<Vec<FireboardApiDevice>>(response_text.as_str());
            // let devices = response.json::<Vec<FireboardApiDevice>>().await;
            if let Err(e) = devices {
                error!("Error parsing devices: {} from response body: {}", e, response_text);
                return Err(e.into());
            }
            Ok(devices.unwrap())
        }
    }

    pub async fn get_realtime_drivelog(
        &self,
        device_uuid: String,
    ) -> Result<Option<FireboardRealtimeDrivelog>> {
        let base_endpoint = self.endpoint()?;
        let endpoint_str = format!(
            "{}/{}/drivelog.json",
            base_endpoint,
            device_uuid
        );
        let endpoint = Url::parse(&endpoint_str)?;
        // let endpoint = base_endpoint.join(format!("/{}/drivelog.json", device_uuid).as_str())?;
        let response = self
            .0
            .client
            .get(endpoint)
            .header(AUTHORIZATION, self.0.auth_header.clone())
            .send()
            .await?;

        let response_text = response.text().await?;

        let v: Value = serde_json::from_str(response_text.as_str())?;
        if v == json!({}) {
            Ok(None)
        } else {
            let json_output =
                serde_json::from_str::<FireboardRealtimeDrivelog>(response_text.as_str())?;
            Ok(Some(json_output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FBX2D: &str = include_str!("../tests/fixtures/devices_fbx2d.json");

    /// reqwest downcasts the preconfigured TLS value to its own `rustls::ClientConfig`
    /// and only fails at `build()` ("unknown TLS backend") if the types differ, which
    /// would happen silently if two rustls versions ever ended up in the lockfile.
    #[test]
    fn shared_rustls_config_is_accepted_by_reqwest() {
        let tls = crate::tls::build_client_config(None).expect("platform roots load");
        let built = reqwest::Client::builder()
            .tls_backend_preconfigured(ClientConfig::clone(&tls))
            .build();
        assert!(built.is_ok(), "{:?}", built.err());
    }

    fn fixture() -> FireboardApiDevice {
        let mut devices: Vec<FireboardApiDevice> = serde_json::from_str(FBX2D).expect("fixture parses");
        assert_eq!(devices.len(), 1);
        devices.remove(0)
    }

    #[test]
    fn live_channels_are_the_current_session_ones() {
        let device = fixture();
        assert_eq!(device.channels.len(), 12, "fixture keeps a sample of the phantom slots");
        let live: Vec<usize> = device.live_channels().map(|c| c.channel).collect();
        assert_eq!(live, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn phantom_slots_are_not_live_even_though_enabled() {
        let device = fixture();
        let phantom = device.channels.iter().find(|c| c.channel == 164).unwrap();
        assert_eq!(phantom.state, None);
        assert!(phantom.last_templog.is_none());
        assert!(!phantom.is_live());
        let null_slot = device.channels.iter().find(|c| c.channel == 0).unwrap();
        assert!(!null_slot.is_live());
    }

    #[test]
    fn a_reading_makes_a_channel_live_regardless_of_state() {
        let channel = FireboardDeviceChannel {
            channel: 9,
            channel_label: "Probe".into(),
            state: None,
            last_templog: Some(FireboardTemps { temp: 72.5 }),
        };
        assert!(channel.is_live());
    }

    #[test]
    fn missing_state_field_deserializes_as_none() {
        let channel: FireboardDeviceChannel =
            serde_json::from_str(r#"{"channel": 3, "channel_label": "Thigh"}"#).unwrap();
        assert_eq!(channel.state, None);
        assert!(!channel.is_live());
    }
}
