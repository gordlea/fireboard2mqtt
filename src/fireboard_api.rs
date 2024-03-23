use anyhow::Result;
use chrono::{DateTime, Local};
use log::error;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Url,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
extern crate serde_json;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, serde::Serialize, Clone)]
pub struct FireboardCloudApiAuthRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}
#[derive(Debug, serde::Deserialize, Clone)]
pub struct FireboardCloudApiAuthResponse {
    pub(crate) key: String,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct FireboardDeviceList {
    pub devices: Vec<FireboardApiDevice>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardApiDevice {
    pub id: usize,
    pub uuid: String,
    pub title: String,
    pub hardware_id: String,
    pub version: String,
    pub channel_count: usize,
    pub degreetype: DegreeType,
    pub model: String,
    pub channels: Vec<FireboardDeviceChannel>,
    pub latest_temps: Vec<FireboardTemps>,
    pub device_log: FireboardDeviceLog,
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
    pub last_templog: Option<FireboardTemps>,
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


#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum DegreeType {
    Celcius = 1,
    Fahrenheit = 2,
}

impl ToString for DegreeType {
    fn to_string(&self) -> String {
        match self {
            DegreeType::Celcius => "°C".to_string(),
            DegreeType::Fahrenheit => "°F".to_string(),
        }
    }
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum DriveModeType {
    Off = 0,
    Manual = 1,
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

impl ToString for DriveModeType {
    fn to_string(&self) -> String {
        match self {
            DriveModeType::Off => "off".to_string(),
            DriveModeType::Manual => "manual".to_string(),
            DriveModeType::Auto => "auto".to_string(),
        }
    }
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum ProgramState {
    NotRunning = 0,
    Running = 1,
    /// I haven't seen this state, i'm guessing paused?
    Paused = 2,
    Complete = 3,
}

pub struct FireboardApiClient {
    api_base: url::Url,
    client: Arc<reqwest::Client>,
}

impl FireboardApiClient {
    pub async fn new(user_email: String, user_password: String) -> Result<FireboardApiClient> {
        let api_base = Url::parse(format!("https://fireboard.io/api/").as_str())?;

        let credentials = FireboardCloudApiAuthRequest {
            username: user_email.to_string(),
            password: user_password.to_string(),
        };

        let auth_client = reqwest::Client::new();
        let auth_result = auth_client
            .post("https://fireboard.io/api/rest-auth/login/")
            .header("Content-Type", "application/json")
            .json(&credentials)
            .send()
            .await;

        if auth_result.is_ok() {
            let auth_response = auth_result.unwrap();

            let auth = match auth_response.error_for_status() {
                Ok(r) => r.json::<FireboardCloudApiAuthResponse>().await?,
                Err(e) => {
                    error!("Error authenticating with Fireboard API! Check your username and password: {}", e.to_string());
                    return Err(e.into());
                }
            };

            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));
            headers.insert(
                "Authorization",
                HeaderValue::from_str(format!("Token {}", auth.key).as_str())?,
            );

            // set default client operation
            let client = Arc::new(
                reqwest::Client::builder()
                    .default_headers(headers)
                    .build()?,
            );

            return Ok(FireboardApiClient { api_base, client });
        } else {
            return Err(anyhow::anyhow!(
                "Error authenticating with Fireboard API! Check your username and password."
            ));
        }
    }

    pub fn devices(&self) -> DevicesEndpoint {
        DevicesEndpoint(self)
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

        let request_attempt = self.0.client.get(endpoint).send().await;

        if let Err(e) = request_attempt {
            error!("Error getting devices: {}", e.to_string());
            return Err(e.into());
        }

        let response = request_attempt.unwrap();

        

        if !response.status().is_success() {
            let status = response.status().to_string();
            error!("Error getting devices: {}", status);
            error!("{}", response.text().await?);
            return Err(anyhow::anyhow!(
                "Error getting devices: {}",
                status
            ));
        } else {
            let response_text = response.text().await?;
            // let v: Value = serde_json::from_str(response_text.as_str())?;
            let devices 
                = serde_json::from_str::<Vec<FireboardApiDevice>>(response_text.as_str());
            // let devices = response.json::<Vec<FireboardApiDevice>>().await;
            if let Err(e) = devices {
                error!("Error parsing devices: {} from response body: {}", e.to_string(), response_text);
                return Err(e.into());
            }
            return Ok(devices.unwrap());
        }
    }

    pub async fn get_realtime_drivelog(
        &self,
        device_uuid: String,
    ) -> Result<Option<FireboardRealtimeDrivelog>> {
        let base_endpoint = self.endpoint()?;
        let endpoint_str = format!(
            "{}/{}/drivelog.json",
            base_endpoint.to_string(),
            device_uuid
        );
        let endpoint = Url::parse(&endpoint_str)?;
        // let endpoint = base_endpoint.join(format!("/{}/drivelog.json", device_uuid).as_str())?;
        let response = self.0.client.get(endpoint).send().await?;

        let response_text = response.text().await?;

        let v: Value = serde_json::from_str(response_text.as_str())?;
        if v == json!({}) {
            return Ok(None);
        } else {
            let json_output =
                serde_json::from_str::<FireboardRealtimeDrivelog>(response_text.as_str())?;
            Ok(Some(json_output))
        }
    }
}
