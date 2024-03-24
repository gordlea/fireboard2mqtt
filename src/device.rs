use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{fireboard_api::DegreeType, constants::{OFFLINE, ONLINE}};

#[derive(PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum UnitOfTemperatureMeasurement {
    // Kelvin = 0,
    Celcius = 1,
    Fahrenheit = 2,
}

impl From<DegreeType> for UnitOfTemperatureMeasurement {
    fn from(degreetype: DegreeType) -> Self {
        match degreetype {
            DegreeType::Celcius => UnitOfTemperatureMeasurement::Celcius,
            DegreeType::Fahrenheit => UnitOfTemperatureMeasurement::Fahrenheit,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FireboardMqttChannel {
    pub availability: String,
    pub state: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoverySensor {
    pub unique_id: String,
    pub object_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub availability: Vec<MQTTDiscoveryAvailabilityEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    pub enabled_by_default: bool,
    pub encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_display_precision: Option<u16>,
    pub qos: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_attributes_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// see https://www.home-assistant.io/integrations/sensor.mqtt/#state_topic
    pub state_topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_of_measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_unit_of_measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<MQTTDiscoveryDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<u32>,
}

impl Into<Bytes> for MQTTDiscoverySensor {
    fn into(self) -> Bytes {
        // let mut sensor = json!(self);
        let json = serde_json::to_string(&self).unwrap();
        // sensor.as_object_mut().unwrap().insert("device".to_string(), json!({}));
        Bytes::from(json)
    }
}

impl Default for MQTTDiscoverySensor {
    fn default() -> Self {
        MQTTDiscoverySensor {
            unique_id: "".to_string(),
            object_id: "".to_string(),
            name: None,
            availability: vec![],
            device_class: None,
            enabled_by_default: true,
            encoding: "utf-8".to_string(),
            suggested_display_precision: None,
            options: None,
            qos: 0,
            state_class: Some("measurement".to_string()),
            json_attributes_topic: None,
            icon: None,
            state_topic: "".to_string(),
            unit_of_measurement: None,
            suggested_unit_of_measurement: None,
            device: None,
            expires_after: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoveryBinarySensor {
    pub unique_id: String,
    pub object_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub availability: Vec<MQTTDiscoveryAvailabilityEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    pub enabled_by_default: bool,
    pub encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_display_precision: Option<u16>,
    pub qos: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_attributes_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// see https://www.home-assistant.io/integrations/sensor.mqtt/#state_topic
    pub state_topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_off: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<MQTTDiscoveryDevice>,
}

impl Into<Bytes> for MQTTDiscoveryBinarySensor {
    fn into(self) -> Bytes {
        // let mut sensor = json!(self);
        let json = serde_json::to_string(&self).unwrap();
        // sensor.as_object_mut().unwrap().insert("device".to_string(), json!({}));
        Bytes::from(json)
    }
}

impl Default for MQTTDiscoveryBinarySensor {
    fn default() -> Self {
        MQTTDiscoveryBinarySensor {
            unique_id: "".to_string(),
            object_id: "".to_string(),
            name: None,
            availability: vec![],
            device_class: None,
            enabled_by_default: true,
            encoding: "utf-8".to_string(),
            suggested_display_precision: None,
            qos: 0,
            json_attributes_topic: None,
            icon: None,
            state_topic: "".to_string(),
            payload_on: None,
            payload_off: None,
            device: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoveryAvailabilityEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_not_available: Option<String>,
    pub topic: String,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub value_template: Option<String>,
}

impl From<String> for MQTTDiscoveryAvailabilityEntry {
    fn from(topic: String) -> Self {
        MQTTDiscoveryAvailabilityEntry {
            payload_available: Some(ONLINE.to_string()),
            payload_not_available: Some(OFFLINE.to_string()),
            topic,
            // value_template: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MQTTDiscoveryDevice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hw_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sw_version: Option<String>,
}

// pub struct MQTTDiscoveryAvailability {
//     pub topic: String,
//     pub payload_available: Option<String>,
//     pub payload_not_available: Option<String>,
// }
