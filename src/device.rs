use bytes::Bytes;
use compact_str::CompactString;
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
    pub availability: CompactString,
    pub state: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoverySensor {
    pub unique_id: CompactString,
    pub object_id: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<CompactString>,
    pub availability: Vec<MQTTDiscoveryAvailabilityEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<CompactString>>,
    pub enabled_by_default: bool,
    pub encoding: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_display_precision: Option<u16>,
    pub qos: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_class: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_attributes_topic: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<CompactString>,
    /// see https://www.home-assistant.io/integrations/sensor.mqtt/#state_topic
    pub state_topic: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_of_measurement: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_unit_of_measurement: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<MQTTDiscoveryDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<u32>,
}

impl From<MQTTDiscoverySensor> for Bytes {
    fn from(sensor: MQTTDiscoverySensor) -> Bytes {
        let json = serde_json::to_string(&sensor).unwrap();
        Bytes::from(json)
    }
}

impl Default for MQTTDiscoverySensor {
    fn default() -> Self {
        MQTTDiscoverySensor {
            unique_id: "".into(),
            object_id: "".into(),
            name: None,
            availability: vec![],
            device_class: None,
            enabled_by_default: true,
            encoding: "utf-8".into(),
            suggested_display_precision: None,
            options: None,
            qos: 0,
            state_class: Some("measurement".into()),
            json_attributes_topic: None,
            icon: None,
            state_topic: "".into(),
            unit_of_measurement: None,
            suggested_unit_of_measurement: None,
            device: None,
            expires_after: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoveryBinarySensor {
    pub unique_id: CompactString,
    pub object_id: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<CompactString>,
    pub availability: Vec<MQTTDiscoveryAvailabilityEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<CompactString>,
    pub enabled_by_default: bool,
    pub encoding: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_display_precision: Option<u16>,
    pub qos: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_attributes_topic: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<CompactString>,
    /// see https://www.home-assistant.io/integrations/sensor.mqtt/#state_topic
    pub state_topic: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_on: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_off: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<MQTTDiscoveryDevice>,
}

impl From<MQTTDiscoveryBinarySensor> for Bytes {
    fn from(sensor: MQTTDiscoveryBinarySensor) -> Bytes {
        // let mut sensor = json!(self);
        let json = serde_json::to_string(&sensor).unwrap();
        // sensor.as_object_mut().unwrap().insert("device".to_string(), json!({}));
        Bytes::from(json)
    }
}

impl Default for MQTTDiscoveryBinarySensor {
    fn default() -> Self {
        MQTTDiscoveryBinarySensor {
            unique_id: "".into(),
            object_id: "".into(),
            name: None,
            availability: vec![],
            device_class: None,
            enabled_by_default: true,
            encoding: "utf-8".into(),
            suggested_display_precision: None,
            qos: 0,
            json_attributes_topic: None,
            icon: None,
            state_topic: "".into(),
            payload_on: None,
            payload_off: None,
            device: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTDiscoveryAvailabilityEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_available: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_not_available: Option<CompactString>,
    pub topic: CompactString,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub value_template: Option<CompactString>,
}

impl From<String> for MQTTDiscoveryAvailabilityEntry {
    fn from(topic: String) -> Self {
        MQTTDiscoveryAvailabilityEntry {
            payload_available: Some(ONLINE.into()),
            payload_not_available: Some(OFFLINE.into()),
            topic: topic.into(),
            // value_template: None,
        }
    }
}


impl From<CompactString> for MQTTDiscoveryAvailabilityEntry {
    fn from(topic: CompactString) -> Self {
        MQTTDiscoveryAvailabilityEntry {
            payload_available: Some(ONLINE.into()),
            payload_not_available: Some(OFFLINE.into()),
            topic,
            // value_template: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MQTTDiscoveryDevice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_url: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<[CompactString; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hw_version: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<CompactString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sw_version: Option<CompactString>,
}

// pub struct MQTTDiscoveryAvailability {
//     pub topic: CompactString,
//     pub payload_available: Option<CompactString>,
//     pub payload_not_available: Option<CompactString>,
// }
