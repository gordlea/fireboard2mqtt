use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveAttributes {
    pub modetype: String,
    pub setpoint: f32,
    pub tiedchannel: usize,
    pub lid_paused: bool,
}

impl From<DriveAttributes> for Bytes {
    fn from(drive_attributes: DriveAttributes) -> Bytes {
        let json = serde_json::to_string(&drive_attributes).unwrap();
        Bytes::from(json)
    }
}