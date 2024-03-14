use anyhow::Result;
use log::error;
use reqwest::{Url, header::{HeaderMap, HeaderValue}};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
extern crate serde_json;
use serde_repr::{Serialize_repr, Deserialize_repr};

#[derive(Debug, serde::Serialize, Clone)]
pub struct FireboardCloudApiAuthRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}
#[derive(Debug, serde::Deserialize, Clone)]
pub struct FireboardCloudApiAuthResponse {
    pub(crate) key: String,
}
// const FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

#[derive(Debug, Serialize, Deserialize)]
pub struct FireboardDeviceList {
    pub devices: Vec<FireboardApiDevice>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardApiDevice {
    pub id: usize,
    #[serde(rename(serialize = "UUID", deserialize = "uuid"))]
    pub uuid: String,
    pub title: String,
    pub created: DateTime<Utc>,
    pub hardware_id: String,
    pub fbj_version: String,
    pub fbn_version: String,
    pub fbu_version: String,
    pub version: String,
    pub probe_config: String,
    pub last_drivelog: Option<DriveLog>,
    pub channel_count: usize,
    pub degreetype: DegreeType,
    pub model: String,
    pub active: bool,
    pub channels: Vec<FireboardDeviceChannel>,
    pub latest_temps: Vec<FireboardTemps>,
    pub device_log: FireboardDeviceLog,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardDeviceLog {
    #[serde(alias = "cpuUsage")]
    pub cpu_usage: String,
    pub nightmode: bool,
    #[serde(alias = "macNIC")]
    pub mac_nic: String,
    // linkquality: usize,
    // #[serde(alias = "linkquality")]
    #[serde(alias = "onboardTemp")]
    pub onboard_temp: f32,
    #[serde(alias = "vBattPer")]
    pub v_batt_per: f32,
    #[serde(alias = "vBattPerRaw")]
    pub v_batt_per_raw: f32,
    #[serde(alias = "vBatt")]
    pub v_batt: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardDeviceChannel {
    pub created: DateTime<Utc>,
    pub alerts: Vec<()>,
    pub enabled: bool,
    pub id: usize,
    pub sessionid: usize,
    pub channel: usize,
    pub channel_label: String,
    pub last_templog: Option<FireboardTemps>,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct FireboardTemps {
    pub temp: f32,
    pub channel: usize,
    pub degreetype: DegreeType,
    pub created: DateTime<Utc>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboardRealtimeDrivelog {
    #[serde(deserialize_with = "drivemode_from_string")]
    pub modetype: DriveModeType,
    pub created: DateTime<Utc>,
    pub device_id: usize,
    pub device_uuid: String,
    pub setpoint: f32,
    pub lidpaused: bool,
    pub created_ms: f32,
    #[serde(deserialize_with = "bool_from_int")]
    pub userinitiated: bool,
    pub degreetype: DegreeType,
    pub powermode: String,
    pub tiedchannel: usize,
    pub driveper: f32,
}

fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where D: Deserializer<'de>
{
    let s = usize::deserialize(deserializer)?;
    Ok(s == 1)
}

fn drivemode_from_string<'de, D>(deserializer: D) -> Result<DriveModeType, D::Error>
    where D: Deserializer<'de>
{
    // eprintln!("drivemode_from_string");
    let s = String::deserialize(deserializer)?;
    // eprintln!("drivemode_from_string: {}", s);
    Ok(DriveModeType::from(s))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriveLog {
    pub created: DateTime<Utc>,
    pub currenttemp: f32,
    pub degreetype: DegreeType,
    pub device_id: usize,
    pub driveper: f32,
    pub drivetype: u8,
    pub id: usize,
    
    // #[serde(deserialize_with = "deserialize_jsonraw")]
    pub jsonraw: String,
    /// 0 is off, 1 is manual, 2 is auto 
    pub modetype: DriveModeType,

    // I don't know what these are
    /// Seconds since program started
    pub pg_elapsed: usize,

    pub pg_state: ProgramState,
    pub pg_step_index: Option<usize>,

    /// progress through the current step? no way to tell what the max is so it's kinda useless
    pub pg_step_position: usize,
    pub pg_step_uuid: Option<String>,
    pub pg_uuid: Option<String>,

    pub powermode: u8,
    pub profiletype: u8,
    pub setpoint: f32,
    pub tiedchannel: usize,
    pub userinitiated: bool,
    pub var1: f32,
    pub var2: f32,
    pub var3: f32,
    pub vbatt: f32
}



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriveModeRaw {
    pub pt: usize,
    #[serde(alias = "pErrSum")]
    pub p_err_sum: f32,
    #[serde(alias = "pOut")]
    pub p_out: f32,
    pub dr: usize,
    pub dt: usize,
    pub sp: usize,
    #[serde(alias = "pLastTime")]
    pub p_last_time: usize,
    pub c: DateTime<Utc>,
    pub d: f32,
    pub mt: usize,
    #[serde(alias = "pStable")]
    pub p_stable: bool,
    #[serde(alias = "pKd")]
    pub p_kd: f32,
    pub vb: f32,
    #[serde(alias = "fanSpeedTarget")]
    pub fan_speed_target: f32,
    pub tc: usize,
    #[serde(alias = "lidPaused")]
    pub lid_paused: bool,
    #[serde(alias = "pKi")]
    pub p_ki: f32,
    pub p: f32,
    pub ct: usize,
    #[serde(alias = "pLastErr")]
    pub p_last_err: f32,
    #[serde(alias = "pKp")]
    pub p_kp: f32,
    pub u: bool,
    pub programinfo: ProgramInfoRaw,
    pub v1: f32,
    pub v2: f32,
    pub v3: f32,
    pub pm: usize
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgramInfoRaw {
    pub elapsed: usize,
    pub stepuuid: Option<String>,
    pub lastupdated: DateTime<Utc>,
    pub state: usize,
    pub uuid: Option<String>,
    pub stepposition: usize,
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
            _ => panic!("Invalid DriveModeType: {}", s)
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
        let auth = auth_client.post("https://fireboard.io/api/rest-auth/login/")
            .header("Content-Type", "application/json")
            .json(&credentials)
            .send()
            .await?
            .json::<FireboardCloudApiAuthResponse>()
            .await?;

        let key = format!("Token {}", auth.key);

        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("Authorization", HeaderValue::from_str(key.as_str())?);

        // set default client operation
        let client = Arc::new(reqwest::Client::builder().default_headers(headers).build()?);

        Ok(FireboardApiClient { api_base, client,})
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


        let request_attempt = self.0.client.get(endpoint)
            .send()
            .await;

        if let Err(e) = request_attempt {
            error!("Error getting devices: {}", e.to_string());
            return Err(e.into());
        }

        let response = request_attempt.unwrap();

        if !response.status().is_success() {
            error!("Error getting devices: {}", response.status().to_string());
            return Err(anyhow::anyhow!("Error getting devices: {}", response.status().to_string()));
        } else {
            let devices = response.json::<Vec<FireboardApiDevice>>().await;
            if let Err(e) = devices {
                error!("Error parsings devices: {}", e.to_string());
                return Err(e.into());
            }
            return Ok(devices.unwrap());
        }
    }

    pub async fn get_realtime_drivelog(&self, device_uuid: String) -> Result<Option<FireboardRealtimeDrivelog>> {
        let base_endpoint = self.endpoint()?;
        let endpoint_str = format!("{}/{}/drivelog.json", base_endpoint.to_string(), device_uuid);
        let endpoint = Url::parse(&endpoint_str)?;
        // let endpoint = base_endpoint.join(format!("/{}/drivelog.json", device_uuid).as_str())?;
        let response = self.0.client.get(endpoint)
            .send()
            .await?;

        let response_text = response.text().await?;

        let v: Value = serde_json::from_str(response_text.as_str())?;
        if v == json!({}) {
            return Ok(None);
        } else {
            let json_output = serde_json::from_str::<FireboardRealtimeDrivelog>(response_text.as_str())?;
            Ok(Some(json_output))
        }
    }
}

