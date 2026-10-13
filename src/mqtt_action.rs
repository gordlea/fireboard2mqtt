use bytes::Bytes;
use rumqttc::v5::mqttbytes::{
    v5::{PublishProperties, SubscribeProperties, UnsubscribeProperties},
    QoS,
};

#[derive(Debug, Clone)]
pub enum MQTTAction {
    // Publish(MQTTPublishAction),
    Publish {
        topic: String,
        qos: QoS,
        retain: bool,
        payload: Bytes,
        props: Option<PublishProperties>,
    },
    Subscribe {
        topic: String,
        qos: QoS,
        props: Option<SubscribeProperties>,
    },
    #[allow(dead_code)]
    Unsubscribe {
        topic: String,
        props: Option<UnsubscribeProperties>,
    },
}

unsafe impl Send for MQTTAction {}
unsafe impl Sync for MQTTAction {}

/// A message the broker delivered to us. Only the fields the watcher needs are carried.
#[derive(Debug, Clone)]
pub struct MQTTInbound {
    pub topic: String,
    pub payload: Bytes,
    /// Set by the broker when this message was replayed from its retained store (i.e. on
    /// subscribe), as opposed to forwarded live. That distinction is what the stale-discovery
    /// cleanup relies on.
    pub retain: bool,
}
