use bytes::Bytes;
use serde::{ Deserialize, Serialize };

#[derive(Debug, Serialize, Deserialize)]
pub struct DriveAttributes {
    pub modetype: String,
    pub setpoint: f32,
    pub tiedchannel: usize,
    pub lid_paused: bool,
}

impl Into<Bytes> for DriveAttributes {
    fn into(self) -> Bytes {
        let json = serde_json::to_string(&self).unwrap();
        Bytes::from(json)
    }
}

