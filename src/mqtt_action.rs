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
    #[allow(dead_code)]
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
