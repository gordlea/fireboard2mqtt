//! # Fireboard Watcher
//! 
//! This module is responsible for watching the Fireboard API and updating the MQTT broker with the latest data
//! as changes occur. It also handles the MQTT discovery process for new devices and channels.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Local;
use rumqttc::v5::mqttbytes::v5::LastWill;
use rumqttc::v5::mqttbytes::QoS;
use tokio::sync::mpsc::{Receiver, Sender};

use anyhow::Result;

use log::{debug, error, info, trace};

use crate::config::Fb2MqttConfig;
use crate::constants::{FIREBOARD_DEVICELOG_UPDATE_INTERVAL_MINUTES, OFF, OFFLINE, ON, ONLINE};
use crate::device::{
    MQTTDiscoveryAvailabilityEntry, MQTTDiscoveryBinarySensor, MQTTDiscoveryDevice,
    MQTTDiscoverySensor,
};
use crate::drive::DriveAttributes;
use crate::fireboard_api::{
    DriveModeType, FireboardApiClient, FireboardApiDevice, FireboardDeviceChannel,
};
use crate::mqtt_action::{MQTTAction, MQTTInbound};
use crate::tls::ClientConfig;
use crate::utils::f32_to_u8_pct;

/// What we last published in a channel's discovery config. If any of it changes the config is
/// re-sent; otherwise it is left alone, because Home Assistant treats every (re)publish of a
/// discovery config as a re-discovery and resets the entity to `unknown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelSignature {
    pub label: String,
    pub unit: String,
}

impl ChannelSignature {
    fn of(device: &FireboardApiDevice, channel: &FireboardDeviceChannel) -> Self {
        ChannelSignature {
            label: channel.channel_label.clone(),
            unit: device.degreetype.to_string(),
        }
    }
}

/// Per-device bookkeeping for the lifetime of this process.
#[derive(Debug, Default)]
pub(crate) struct DeviceDiscoveryState {
    /// We asked the broker for this device's discovery topics (so retained configs get replayed).
    subscribed: bool,
    /// Battery and drive discovery configs have been published.
    device_level_published: bool,
    /// Channels whose discovery config we have published this run, with what we published.
    channels: HashMap<usize, ChannelSignature>,
    /// Channel discovery configs the broker holds as retained messages for this device.
    retained_in_broker: HashSet<usize>,
}

impl DeviceDiscoveryState {
    /// The live channels whose discovery config is missing or out of date.
    fn channels_to_publish<'a>(
        &self,
        device: &'a FireboardApiDevice,
    ) -> Vec<&'a FireboardDeviceChannel> {
        device
            .live_channels()
            .filter(|channel| {
                self.channels.get(&channel.channel) != Some(&ChannelSignature::of(device, channel))
            })
            .collect()
    }

    fn mark_channel_published(
        &mut self,
        device: &FireboardApiDevice,
        channel: &FireboardDeviceChannel,
    ) {
        self.channels
            .insert(channel.channel, ChannelSignature::of(device, channel));
    }

    /// Retained channel configs in the broker that belong to no live channel.
    ///
    /// A channel is never cleaned once this run has published it, so a channel that reported a
    /// temperature earlier today survives the device going quiet.
    fn channels_to_clean(&self, device: &FireboardApiDevice) -> Vec<usize> {
        let live: HashSet<usize> = device.live_channels().map(|c| c.channel).collect();
        let mut stale: Vec<usize> = self
            .retained_in_broker
            .iter()
            .copied()
            .filter(|channel| !self.channels.contains_key(channel) && !live.contains(channel))
            .collect();
        stale.sort_unstable();
        stale
    }
}

/// Reads `<discovery_prefix>/sensor/<hardware_id>/channel_<n>/config` back into its parts.
/// Anything else (battery, drive, other prefixes, other integrations) yields `None`.
pub(crate) fn parse_channel_config_topic(
    discovery_prefix: &str,
    topic: &str,
) -> Option<(String, usize)> {
    let rest = topic.strip_prefix(discovery_prefix)?.strip_prefix('/')?;
    let mut parts = rest.split('/');
    if parts.next()? != "sensor" {
        return None;
    }
    let hardware_id = parts.next()?;
    let channel_part = parts.next()?;
    if parts.next()? != "config" || parts.next().is_some() || hardware_id.is_empty() {
        return None;
    }
    let channel = channel_part.strip_prefix("channel_")?.parse::<usize>().ok()?;
    Some((hardware_id.to_string(), channel))
}

pub struct FireboardWatcher {
    online_device_count: u8,
    fb_client: FireboardApiClient,
    tx: Sender<MQTTAction>,
    rx_inbound: Receiver<MQTTInbound>,
    cfg: Fb2MqttConfig,
    discovery: HashMap<String, DeviceDiscoveryState>,
}

impl FireboardWatcher {
    pub async fn new(
        cfg: &Fb2MqttConfig,
        tls: &Arc<ClientConfig>,
        tx: Sender<MQTTAction>,
        rx_inbound: Receiver<MQTTInbound>,
    ) -> Result<FireboardWatcher> {
        let fb_client = FireboardApiClient::new(
            cfg.fireboardaccount_email.clone(),
            cfg.fireboardaccount_password.clone(),
            tls,
        )
        .await?;
        debug!("client authenticated successfully");

        let mut fb_watcher = FireboardWatcher {
            online_device_count: 0,
            fb_client,
            tx,
            rx_inbound,
            cfg: cfg.clone(),
            discovery: HashMap::new(),
        };
        fb_watcher.init().await;
        Ok(fb_watcher)
    }

    #[cfg(test)]
    fn for_test(cfg: Fb2MqttConfig) -> (FireboardWatcher, Receiver<MQTTAction>) {
        let (tx, rx_actions) = tokio::sync::mpsc::channel(64);
        let (_tx_inbound, rx_inbound) = tokio::sync::mpsc::channel(8);
        let watcher = FireboardWatcher {
            online_device_count: 0,
            fb_client: FireboardApiClient::unauthenticated(),
            tx,
            rx_inbound,
            cfg,
            discovery: HashMap::new(),
        };
        (watcher, rx_actions)
    }

    pub fn online_device_count(&self) -> u8 {
        self.online_device_count
    }

    pub fn get_topic_bridge_availablility(&self) -> String {
        format!("{}/bridge/availability", self.cfg.mqtt_base_topic)
    }

    pub fn get_discovery_sensor_base_topic(&self, device_identifier: &String) -> String {
        format!(
            "{}/sensor/{}",
            self.cfg.mqtt_discovery_topic, device_identifier
        )
    }

    pub fn get_discovery_binary_sensor_base_topic(&self, device_identifier: &String) -> String {
        format!(
            "{}/binary_sensor/{}",
            self.cfg.mqtt_discovery_topic, device_identifier
        )
    }

    pub fn get_device_base_topic(&self, device_identifier: &String) -> String {
        format!("{}/{}", self.cfg.mqtt_base_topic, device_identifier)
    }

    pub fn get_topic_device_availablility(&self, device_identifier: &String) -> String {
        format!(
            "{}/availability",
            self.get_device_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_battery(&self, device_identifier: &String) -> String {
        format!("{}/battery", self.get_device_base_topic(device_identifier))
    }

    pub fn get_topic_device_battery_discovery(&self, device_identifier: &String) -> String {
        format!(
            "{}/battery/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_channel(&self, device_identifier: &String, channel: &usize) -> String {
        format!(
            "{}/channel_{}",
            self.get_device_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_channel_availability(
        &self,
        device_identifier: &String,
        channel: &usize,
    ) -> String {
        format!(
            "{}/channel_{}/availability",
            self.get_device_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_channel_discovery(
        &self,
        device_identifier: &String,
        channel: &usize,
    ) -> String {
        format!(
            "{}/channel_{}/config",
            self.get_discovery_sensor_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_drive_discovery(&self, device_identifier: &String) -> String {
        format!(
            "{}/drive/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drivemode_discovery(&self, device_identifier: &String) -> String {
        format!(
            "{}/drivemode/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive_setpoint_discovery(&self, device_identifier: &String) -> String {
        format!(
            "{}/drive_setpoint/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive_lidpaused_discovery(&self, device_identifier: &String) -> String {
        format!(
            "{}/drive_lidpaused/config",
            self.get_discovery_binary_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive(&self, device_identifier: &String) -> String {
        format!("{}/drive", self.get_device_base_topic(device_identifier))
    }

    pub fn get_topic_device_drive_availability(&self, device_identifier: &String) -> String {
        format!(
            "{}/availability",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_setpoint_availability(
        &self,
        device_identifier: &String,
    ) -> String {
        format!(
            "{}/setpoint_availability",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_state(&self, device_identifier: &String) -> String {
        format!("{}/state", self.get_topic_device_drive(device_identifier))
    }

    pub fn get_topic_device_drive_mode(&self, device_identifier: &String) -> String {
        format!("{}/mode", self.get_topic_device_drive(device_identifier))
    }

    pub fn get_topic_device_drive_setpoint(&self, device_identifier: &String) -> String {
        format!(
            "{}/setpoint",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_lidpaused(&self, device_identifier: &String) -> String {
        format!(
            "{}/lidpaused",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_attributes(&self, device_identifier: &String) -> String {
        format!(
            "{}/attributes",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_last_will(&self) -> LastWill {
        let topic = self.get_topic_bridge_availablility();
        LastWill {
            topic: topic.into(),
            message: OFFLINE.into(),
            qos: QoS::AtLeastOnce,
            retain: true,
            properties: None,
        }
    }

    async fn init(&mut self) {
        // first we will set the availability of the bridge (this service)

        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_bridge_availablility(),
                qos: QoS::AtLeastOnce,
                retain: true,
                payload: ONLINE.into(),
                props: None,
            })
            .await
            .unwrap();
    }

    /// Publish discovery configs for anything new or changed on this device. Nothing is
    /// re-sent on an ordinary poll.
    async fn update_discovery(&mut self, device: &FireboardApiDevice) {
        let hardware_id = device.hardware_id.clone();
        let parent_device = Self::build_parent_device(device);

        let state = self.discovery.entry(hardware_id.clone()).or_default();
        let publish_device_level = !state.device_level_published;
        let channels_to_publish: Vec<FireboardDeviceChannel> = state
            .channels_to_publish(device)
            .into_iter()
            .cloned()
            .collect();

        if publish_device_level {
            self.publish_device_level_discovery(device, &parent_device)
                .await;
        }
        for channel in &channels_to_publish {
            self.publish_channel_discovery(device, channel, &parent_device)
                .await;
        }

        let state = self.discovery.entry(hardware_id).or_default();
        state.device_level_published = true;
        for channel in &channels_to_publish {
            state.mark_channel_published(device, channel);
        }
    }

    fn build_parent_device(device: &FireboardApiDevice) -> Option<MQTTDiscoveryDevice> {
        let connections = device
            .device_log
            .as_ref()
            .map(|device_log| vec![["mac".to_string(), device_log.mac_nic.clone()]]);
        Some(MQTTDiscoveryDevice {
            configuration_url: Some(
                format!("https://fireboard.io/devices/{}/edit/", device.id).to_string(),
            ),
            connections,
            identifiers: Some(vec![
                device.id.to_string(),
                device.hardware_id.clone(),
                device.uuid.clone(),
            ]),
            manufacturer: Some("Fireboard Labs".to_string()),
            model: Some(device.model.clone()),
            name: Some(device.title.clone()),
            serial_number: Some(device.hardware_id.clone()),
            sw_version: Some(device.version.clone()),
            ..MQTTDiscoveryDevice::default()
        })
    }

    /// The discovery config for one channel. Kept separate so tests can check its shape.
    fn channel_discovery_sensor(
        &self,
        device: &FireboardApiDevice,
        channel: &FireboardDeviceChannel,
        parent_device: &Option<MQTTDiscoveryDevice>,
    ) -> MQTTDiscoverySensor {
        let hardware_id = &device.hardware_id;
        let channel_id = format!("{}_channel_{}", hardware_id, channel.channel);
        let channel_topic = self.get_topic_device_channel(hardware_id, &channel.channel);

        MQTTDiscoverySensor {
            unique_id: channel_id.clone(),
            default_entity_id: format!("sensor.{}", channel_id),
            name: Some(channel.channel_label.clone()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_channel_availability(hardware_id, &channel.channel),
                ),
            ],
            suggested_unit_of_measurement: Some("°F".to_string()),
            device_class: Some("temperature".to_string()),
            qos: 0,
            icon: None,
            state_topic: format!("{}/state", channel_topic),
            unit_of_measurement: Some(device.degreetype.to_string()),
            device: parent_device.clone(),
            // TODO make this configurable?
            expires_after: Some(600),
            ..MQTTDiscoverySensor::default()
        }
    }

    async fn publish_channel_discovery(
        &self,
        device: &FireboardApiDevice,
        channel: &FireboardDeviceChannel,
        parent_device: &Option<MQTTDiscoveryDevice>,
    ) {
        info!(
            "publishing discovery for {} channel {} ({})",
            device.hardware_id, channel.channel, channel.channel_label
        );
        let channel_discovery = self.channel_discovery_sensor(device, channel, parent_device);
        self.tx
            .send(MQTTAction::Publish {
                topic: self
                    .get_topic_device_channel_discovery(&device.hardware_id, &channel.channel),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: channel_discovery.into(),
                props: None,
            })
            .await
            .unwrap();
    }

    /// Battery and drive discovery configs. Published once per device per run.
    async fn publish_device_level_discovery(
        &self,
        device: &FireboardApiDevice,
        parent_device: &Option<MQTTDiscoveryDevice>,
    ) {
        let hardware_id = device.hardware_id.clone();
        info!("publishing device discovery for {}", hardware_id);

        // set battery mqtt discovery
        let battery_id = format!("{}_battery", hardware_id);
        let battery_discovery = MQTTDiscoverySensor {
            unique_id: battery_id.clone(),
            default_entity_id: format!("sensor.{}", battery_id),
            name: Some("Battery".to_string()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
            ],
            device_class: Some("battery".to_string()),
            qos: 0,
            icon: None,
            state_topic: self.get_topic_device_battery(&hardware_id),
            unit_of_measurement: Some("%".to_string()),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_battery_discovery(&hardware_id),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: battery_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        // if drive_enabled {
        // set drive mqtt discovery
        let drive_id = format!("{}_drive", hardware_id);
        let drive_discovery = MQTTDiscoverySensor {
            unique_id: drive_id.clone(),
            default_entity_id: format!("sensor.{}", drive_id),
            name: Some("Drive".to_string()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_availability(&hardware_id),
                ),
            ],
            device_class: None,
            qos: 0,
            icon: Some("mdi:fan".to_string()),
            // TODO make this configurable?
            expires_after: Some(600),
            state_topic: self.get_topic_device_drive_state(&hardware_id),
            unit_of_measurement: Some("%".to_string()),
            device: parent_device.clone(),
            json_attributes_topic: Some(self.get_topic_device_drive_attributes(&hardware_id)),
            ..MQTTDiscoverySensor::default()
        };

        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drive_discovery(&hardware_id),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_mode_id = format!("{}_mode", drive_id.clone());
        let drive_mode_discovery = MQTTDiscoverySensor {
            unique_id: drive_mode_id.clone(),
            default_entity_id: format!("sensor.{}", drive_mode_id),
            name: Some("Drive Mode".to_string()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_availability(&hardware_id),
                ),
            ],
            device_class: Some("enum".to_string()),
            options: Some(vec![
                "off".to_string(),
                "manual".to_string(),
                "auto".to_string(),
            ]),
            qos: 0,
            icon: Some("mdi:fan-alert".to_string()),
            state_class: None,
            // icon: None,
            state_topic: self.get_topic_device_drive_mode(&hardware_id),
            // unit_of_measurement: Some("%".to_string()),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drivemode_discovery(&hardware_id),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_mode_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_setpoint_id = format!("{}_setpoint", drive_id.clone());
        let drive_setpoint_discovery = MQTTDiscoverySensor {
            unique_id: drive_setpoint_id.clone(),
            default_entity_id: format!("sensor.{}", drive_setpoint_id),
            name: Some("Drive Setpoint".to_string()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_availability(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_setpoint_availability(&hardware_id),
                ),
            ],
            icon: Some("mdi:thermometer-auto".to_string()),
            suggested_unit_of_measurement: Some("°F".to_string()),
            device_class: Some("temperature".to_string()),
            qos: 0,
            state_topic: self.get_topic_device_drive_setpoint(&hardware_id),
            unit_of_measurement: Some(device.degreetype.to_string()),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drive_setpoint_discovery(&hardware_id),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_setpoint_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_lidpaused_id = format!("{}_lidpaused", drive_id.clone());
        let drive_lidpaused_discovery = MQTTDiscoveryBinarySensor {
            unique_id: drive_lidpaused_id.clone(),
            default_entity_id: format!("binary_sensor.{}", drive_lidpaused_id),
            name: Some("Drive Lid Paused".to_string()),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_availability(&hardware_id),
                ),
            ],
            // icon: Some("mdi:fan-alert".to_string()),
            // device_class: Some("opening".to_string()),
            qos: 0,
            state_topic: self.get_topic_device_drive_lidpaused(&hardware_id),
            device: parent_device.clone(),
            payload_on: Some(ON.into()),
            payload_off: Some(OFF.into()),
            ..MQTTDiscoveryBinarySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drive_lidpaused_discovery(&hardware_id),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_lidpaused_discovery.into(),
                props: None,
            })
            .await
            .unwrap();
    }

    /// Record every retained discovery config the broker has replayed to us since the last poll.
    fn drain_inbound(&mut self) {
        while let Ok(message) = self.rx_inbound.try_recv() {
            if !message.retain || message.payload.is_empty() {
                continue;
            }
            if let Some((hardware_id, channel)) =
                parse_channel_config_topic(&self.cfg.mqtt_discovery_topic, &message.topic)
            {
                trace!(
                    "broker holds retained discovery for {} channel {}",
                    hardware_id,
                    channel
                );
                self.discovery
                    .entry(hardware_id)
                    .or_default()
                    .retained_in_broker
                    .insert(channel);
            }
        }
    }

    /// Ask the broker to replay this device's retained discovery configs (once per run).
    async fn ensure_subscribed(&mut self, hardware_id: &String) {
        let state = self.discovery.entry(hardware_id.clone()).or_default();
        if state.subscribed {
            return;
        }
        state.subscribed = true;
        // `+` has to be a whole topic level, so `channel_+` is not expressible; the parser
        // filters out the battery and drive configs this also matches.
        let topic = format!("{}/+/config", self.get_discovery_sensor_base_topic(hardware_id));
        debug!("subscribing to {} to find stale discovery configs", topic);
        self.tx
            .send(MQTTAction::Subscribe {
                topic,
                qos: QoS::AtLeastOnce,
                props: None,
            })
            .await
            .unwrap();
    }

    /// Delete retained discovery configs for channels this device does not have.
    async fn cleanup_stale_channels(&mut self, device: &FireboardApiDevice) {
        let hardware_id = device.hardware_id.clone();
        let stale = match self.discovery.get(&hardware_id) {
            Some(state) => state.channels_to_clean(device),
            None => return,
        };
        for channel in &stale {
            info!(
                "removing stale discovery config for {} channel {}",
                hardware_id, channel
            );
            for topic in [
                self.get_topic_device_channel_discovery(&hardware_id, channel),
                self.get_topic_device_channel_availability(&hardware_id, channel),
            ] {
                self.tx
                    .send(MQTTAction::Publish {
                        topic,
                        qos: QoS::AtLeastOnce,
                        retain: true,
                        payload: "".into(),
                        props: None,
                    })
                    .await
                    .unwrap();
            }
        }
        if let Some(state) = self.discovery.get_mut(&hardware_id) {
            for channel in &stale {
                state.retained_in_broker.remove(channel);
            }
        }
    }

    pub async fn update(&mut self) {
        info!("checking fireboard api for updates");
        self.drain_inbound();
        let drive_enabled = self.cfg.fireboard_enable_drive;
        let result = self.fb_client.devices().list().await;
        if let Ok(returned_devices) = result {
            info!("{} devices fetched successfully", returned_devices.len());
            #[cfg(feature = "pretty_print_json_logs")]
            trace!(
                "devices fetched successfully: {}",
                serde_json::to_string_pretty(&returned_devices).unwrap()
            );
            #[cfg(not(feature = "pretty_print_json_logs"))]
            trace!("devices fetched successfully: {:?}", returned_devices);

            self.online_device_count = 0;

            for device in returned_devices {
                let hardware_id = device.hardware_id.clone();

                debug!("found device: {:?}", hardware_id);
                self.ensure_subscribed(&hardware_id).await;

                let latest_temps = device.latest_temps.clone();
                let device_online = {
                    let has_latest_temps = !latest_temps.is_empty();
                    if has_latest_temps {
                        true
                    } else if let Some(device_log) = &device.device_log {
                        let now = Local::now();
                        let diff = now - device_log.date;
                        diff.num_minutes() < FIREBOARD_DEVICELOG_UPDATE_INTERVAL_MINUTES
                    } else {
                        false
                    }
                };

                // set device availability
                self.tx
                    .send(MQTTAction::Publish {
                        topic: self.get_topic_device_availablility(&hardware_id),
                        qos: QoS::AtLeastOnce,
                        retain: true,
                        payload: if device_online {
                            ONLINE.into()
                        } else {
                            OFFLINE.into()
                        },
                        props: None,
                    })
                    .await
                    .unwrap();

                // update mqtt discovery
                self.update_discovery(&device).await;

                // set battery state
                if device_online {
                    self.online_device_count += 1;
                    if let Some(device_log) = &device.device_log {
                        let batt_percentage = f32_to_u8_pct(device_log.v_batt_per);
                        self.tx
                            .send(MQTTAction::Publish {
                                topic: self.get_topic_device_battery(&hardware_id),
                                qos: QoS::AtMostOnce,
                                retain: true,
                                payload: format!("{batt_percentage}").into(),
                                props: None,
                            })
                            .await
                            .unwrap();                        
                    }


                }

                if device_online {
                    // do channel temperatures
                    for channel in device.live_channels() {
                        // let unique_id = format!("{}_{}", device.hardware_id.clone(), channel.channel);
                        let channel_topic =
                            self.get_topic_device_channel(&hardware_id, &channel.channel);

                        // channel availability
                        self.tx
                            .send(MQTTAction::Publish {
                                topic: format!("{}/availability", channel_topic),
                                qos: QoS::AtLeastOnce,
                                retain: true,
                                payload: if channel.last_templog.is_some() {
                                    ONLINE.into()
                                } else {
                                    OFFLINE.into()
                                },
                                props: None,
                            })
                            .await
                            .unwrap();

                        if let Some(templog) = &channel.last_templog {
                            // channel is online
                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: format!("{}/state", channel_topic),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: templog.temp.to_string().into(),
                                    props: None,
                                })
                                .await
                                .unwrap();
                        } else {
                            // channel is offline
                            // self.tx
                            //     .send(MQTTAction::Publish {
                            //         topic: format!("{}/state", channel_topic),
                            //         qos: QoS::AtMostOnce,
                            //         retain: false,
                            //         payload: "".into(),
                            //         props: None,
                            //     })
                            //     .await
                            //     .unwrap();
                        }
                    }
                }

                if drive_enabled {
                    let rt_drivelog_request = self
                        .fb_client
                        .devices()
                        .get_realtime_drivelog(device.uuid.clone())
                        .await;
                    if let Ok(rt_drivelog) = rt_drivelog_request {
                        if let Some(drivelog) = &rt_drivelog {
                            // drive not available
                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_availability(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: ONLINE.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            debug!("drivelog: {:?}", drivelog);
                            let modetype = if drivelog.setpoint >= 100.0 {
                                DriveModeType::Auto
                            } else if drivelog.driveper > 0.0 {
                                DriveModeType::Manual
                            } else {
                                DriveModeType::Off
                            };

                            debug!("drivelog modetype: {:?}", modetype);

                            let state = f32_to_u8_pct(drivelog.driveper);
                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_state(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: state.to_string().into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            let drive_attributes = DriveAttributes {
                                modetype: modetype.to_string(),
                                setpoint: drivelog.setpoint,
                                tiedchannel: drivelog.tiedchannel,
                                lid_paused: drivelog.lidpaused,
                            };
                            if modetype == DriveModeType::Auto {
                                self.tx
                                    .send(MQTTAction::Publish {
                                        topic: self.get_topic_device_drive_setpoint(&hardware_id),
                                        qos: QoS::AtMostOnce,
                                        retain: false,
                                        payload: drivelog.setpoint.to_string().into(),
                                        props: None,
                                    })
                                    .await
                                    .unwrap();
                                self.tx
                                    .send(MQTTAction::Publish {
                                        topic: self.get_topic_device_drive_setpoint_availability(
                                            &hardware_id,
                                        ),
                                        qos: QoS::AtMostOnce,
                                        retain: false,
                                        payload: ONLINE.into(),
                                        props: None,
                                    })
                                    .await
                                    .unwrap();
                            } else {
                                self.tx
                                    .send(MQTTAction::Publish {
                                        topic: self.get_topic_device_drive_setpoint(&hardware_id),
                                        qos: QoS::AtMostOnce,
                                        retain: false,
                                        payload: "".into(),
                                        props: None,
                                    })
                                    .await
                                    .unwrap();
                                self.tx
                                    .send(MQTTAction::Publish {
                                        topic: self.get_topic_device_drive_setpoint_availability(
                                            &hardware_id,
                                        ),
                                        qos: QoS::AtMostOnce,
                                        retain: false,
                                        payload: OFFLINE.into(),
                                        props: None,
                                    })
                                    .await
                                    .unwrap();
                            }
                            // self.tx
                            // .send(MQTTAction::Publish {
                            //     topic: self.get_topic_device_drive_setpoint(&hardware_id),
                            //     qos: QoS::AtMostOnce,
                            //     retain: false,
                            //     payload: drivelog.setpoint.to_string().into(),
                            //     props: None,
                            // }).await
                            // .unwrap();

                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_lidpaused(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: if drivelog.lidpaused { ON } else { OFF }.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_attributes(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: drive_attributes.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_mode(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: modetype.to_string().into(),
                                    props: None,
                                })
                                .await
                                .unwrap();
                        } else {
                            // drive not available
                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_availability(&hardware_id),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: OFFLINE.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            // self.tx
                            //     .send(MQTTAction::Publish {
                            //         topic: self.get_topic_device_drive_state(&hardware_id),
                            //         qos: QoS::AtMostOnce,
                            //         retain: false,
                            //         payload: "".into(),
                            //         props: None,
                            //     })
                            //     .await
                            //     .unwrap();

                            // self.tx
                            //     .send(MQTTAction::Publish {
                            //         topic: self.get_topic_device_drive_attributes(&hardware_id),
                            //         qos: QoS::AtMostOnce,
                            //         retain: false,
                            //         payload: "".into(),
                            //         props: None,
                            //     })
                            //     .await
                            //     .unwrap();
                        }
                    } else if let Err(err) = rt_drivelog_request {
                        error!("Error fetching realtime drivelog: {:?}", err);
                    }
                } else {
                    self.tx
                        .send(MQTTAction::Publish {
                            topic: self.get_topic_device_drive_availability(&hardware_id),
                            qos: QoS::AtMostOnce,
                            retain: true,
                            payload: OFFLINE.into(),
                            props: None,
                        })
                        .await
                        .unwrap();
                }

                self.cleanup_stale_channels(&device).await;
            }
        } else if let Err(err) = result {
            error!("Error fetching devices: {:?}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fireboard_api::{DegreeType, FireboardTemps};

    const FBX2D: &str = include_str!("../tests/fixtures/devices_fbx2d.json");

    fn fixture() -> FireboardApiDevice {
        let mut devices: Vec<FireboardApiDevice> = serde_json::from_str(FBX2D).unwrap();
        devices.remove(0)
    }

    fn cfg(discovery_prefix: &str) -> Fb2MqttConfig {
        Fb2MqttConfig {
            fireboardaccount_email: "test@example.com".into(),
            fireboardaccount_password: "secret".into(),
            fireboard_enable_drive: false,
            fireboard_api_device_online_update_interval_seconds: 30,
            fireboard_api_device_offline_update_interval_seconds: 90,
            mqtt_host: "localhost".into(),
            mqtt_port: 1883,
            mqtt_use_tls: false,
            mqtt_tls_ca_file: None,
            mqtt_discovery_topic: discovery_prefix.into(),
            mqtt_base_topic: "fireboard2mqtt".into(),
            mqtt_credentials: None,
            mqtt_clientid: "fireboard2mqtt".into(),
        }
    }

    fn live_channel_numbers(device: &FireboardApiDevice) -> Vec<usize> {
        device.live_channels().map(|c| c.channel).collect()
    }

    // --- topic parsing ---------------------------------------------------------------------

    #[test]
    fn channel_config_topic_round_trips_through_the_topic_builders() {
        for prefix in ["homeassistant", "ha/discovery"] {
            let (watcher, _rx) = FireboardWatcher::for_test(cfg(prefix));
            for (hardware_id, channel) in [("GHK4F6CD2", 1usize), ("ABC123", 164)] {
                let topic =
                    watcher.get_topic_device_channel_discovery(&hardware_id.to_string(), &channel);
                assert_eq!(
                    parse_channel_config_topic(prefix, &topic),
                    Some((hardware_id.to_string(), channel)),
                    "{topic}"
                );
            }
        }
    }

    #[test]
    fn non_channel_config_topics_are_ignored() {
        let (watcher, _rx) = FireboardWatcher::for_test(cfg("homeassistant"));
        let hardware_id = "GHK4F6CD2".to_string();
        let rejected = [
            watcher.get_topic_device_battery_discovery(&hardware_id),
            watcher.get_topic_device_drive_discovery(&hardware_id),
            watcher.get_topic_device_drive_lidpaused_discovery(&hardware_id),
            watcher.get_topic_device_channel_availability(&hardware_id, &3),
            "homeassistant/sensor/GHK4F6CD2/channel_x/config".to_string(),
            "homeassistant/sensor/GHK4F6CD2/channel_3/config/extra".to_string(),
            "homeassistant/sensor//channel_3/config".to_string(),
            "other/sensor/GHK4F6CD2/channel_3/config".to_string(),
            "zigbee2mqtt/sensor/GHK4F6CD2/channel_3/config".to_string(),
        ];
        for topic in rejected {
            assert_eq!(parse_channel_config_topic("homeassistant", &topic), None, "{topic}");
        }
    }

    // --- what to publish --------------------------------------------------------------------

    #[test]
    fn first_poll_publishes_every_live_channel_and_nothing_else() {
        let device = fixture();
        let state = DeviceDiscoveryState::default();
        let to_publish: Vec<usize> = state
            .channels_to_publish(&device)
            .iter()
            .map(|c| c.channel)
            .collect();
        assert_eq!(to_publish, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn an_unchanged_poll_publishes_nothing() {
        let device = fixture();
        let mut state = DeviceDiscoveryState::default();
        for channel in device.live_channels() {
            state.mark_channel_published(&device, channel);
        }
        assert!(state.channels_to_publish(&device).is_empty());
    }

    #[test]
    fn a_relabelled_channel_is_republished_alone() {
        let mut device = fixture();
        let mut state = DeviceDiscoveryState::default();
        for channel in device.live_channels() {
            state.mark_channel_published(&device, channel);
        }
        device.channels[2].channel_label = "Brisket".into(); // channel 2
        let to_publish: Vec<usize> = state
            .channels_to_publish(&device)
            .iter()
            .map(|c| c.channel)
            .collect();
        assert_eq!(to_publish, vec![2]);
    }

    #[test]
    fn a_unit_change_republishes_all_live_channels() {
        let mut device = fixture();
        let mut state = DeviceDiscoveryState::default();
        for channel in device.live_channels() {
            state.mark_channel_published(&device, channel);
        }
        device.degreetype = DegreeType::Celcius;
        let to_publish: Vec<usize> = state
            .channels_to_publish(&device)
            .iter()
            .map(|c| c.channel)
            .collect();
        assert_eq!(to_publish, live_channel_numbers(&device));
    }

    // --- what to clean ----------------------------------------------------------------------

    #[test]
    fn retained_phantom_channels_are_cleaned() {
        let device = fixture();
        let mut state = DeviceDiscoveryState::default();
        for channel in device.live_channels() {
            state.mark_channel_published(&device, channel);
        }
        state.retained_in_broker = [0usize, 1, 2, 7, 10, 164].into_iter().collect();
        assert_eq!(state.channels_to_clean(&device), vec![0, 7, 10, 164]);
    }

    #[test]
    fn a_channel_seen_live_this_run_is_never_cleaned() {
        let mut device = fixture();
        let mut state = DeviceDiscoveryState::default();
        for channel in device.live_channels() {
            state.mark_channel_published(&device, channel);
        }
        // The API later stops flagging channel 4 as live (e.g. session ended).
        device.channels[4].state = None; // channel 4
        assert!(!device.channels[4].is_live());
        state.retained_in_broker = [4usize, 7].into_iter().collect();
        assert_eq!(state.channels_to_clean(&device), vec![7]);
    }

    #[test]
    fn a_retained_live_channel_not_yet_published_is_kept() {
        let device = fixture();
        let state = DeviceDiscoveryState {
            retained_in_broker: [3usize, 7].into_iter().collect(),
            ..Default::default()
        };
        assert_eq!(state.channels_to_clean(&device), vec![7]);
    }

    #[test]
    fn a_channel_with_a_reading_but_no_state_flag_is_kept() {
        let mut device = fixture();
        device.channels[7].last_templog = Some(FireboardTemps { temp: 225.0 }); // channel 7
        let state = DeviceDiscoveryState {
            retained_in_broker: [7usize, 8].into_iter().collect(),
            ..Default::default()
        };
        assert_eq!(state.channels_to_clean(&device), vec![8]);
        let to_publish: Vec<usize> = state
            .channels_to_publish(&device)
            .iter()
            .map(|c| c.channel)
            .collect();
        assert_eq!(to_publish, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    // --- discovery payload shape ------------------------------------------------------------

    #[test]
    fn channel_discovery_payload_keeps_the_existing_entity_identity() {
        let (watcher, _rx) = FireboardWatcher::for_test(cfg("homeassistant"));
        let device = fixture();
        let channel = device.channels.iter().find(|c| c.channel == 2).unwrap();
        let parent = FireboardWatcher::build_parent_device(&device);
        let sensor = watcher.channel_discovery_sensor(&device, channel, &parent);

        assert_eq!(sensor.unique_id, "GHK4F6CD2_channel_2");
        assert_eq!(sensor.default_entity_id, "sensor.GHK4F6CD2_channel_2");
        assert_eq!(sensor.name.as_deref(), Some("Breast"));
        assert_eq!(sensor.state_topic, "fireboard2mqtt/GHK4F6CD2/channel_2/state");
        assert_eq!(sensor.unit_of_measurement.as_deref(), Some("°F"));
        assert_eq!(sensor.expires_after, Some(600));
        let availability: Vec<&str> = sensor.availability.iter().map(|a| a.topic.as_str()).collect();
        assert_eq!(
            availability,
            vec![
                "fireboard2mqtt/bridge/availability",
                "fireboard2mqtt/GHK4F6CD2/availability",
                "fireboard2mqtt/GHK4F6CD2/channel_2/availability",
            ]
        );
        let device_block = sensor.device.as_ref().unwrap();
        assert_eq!(device_block.serial_number.as_deref(), Some("GHK4F6CD2"));
        assert_eq!(device_block.model.as_deref(), Some("FBX2D"));

        let json: serde_json::Value = serde_json::from_slice(&bytes::Bytes::from(sensor)).unwrap();
        assert_eq!(json["default_entity_id"], "sensor.GHK4F6CD2_channel_2");
        assert_eq!(json["unique_id"], "GHK4F6CD2_channel_2");
    }

    // --- end to end through the watcher (no broker, no API) --------------------------------

    fn drain(rx: &mut Receiver<MQTTAction>) -> Vec<MQTTAction> {
        let mut actions = vec![];
        while let Ok(action) = rx.try_recv() {
            actions.push(action);
        }
        actions
    }

    fn published_topics(actions: &[MQTTAction]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                MQTTAction::Publish { topic, .. } => Some(topic.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn discovery_goes_out_once_per_run() {
        let (mut watcher, mut rx) = FireboardWatcher::for_test(cfg("homeassistant"));
        let device = fixture();

        watcher.update_discovery(&device).await;
        let first = drain(&mut rx);
        let topics = published_topics(&first);
        // battery + drive + drive mode + drive setpoint + drive lid paused, then six channels
        assert_eq!(topics.len(), 5 + 6, "{topics:?}");
        for channel in 1..=6 {
            assert!(topics.contains(&format!(
                "homeassistant/sensor/GHK4F6CD2/channel_{channel}/config"
            )));
        }
        assert!(!topics.iter().any(|t| t.contains("channel_0/") || t.contains("channel_164/")));
        for action in &first {
            if let MQTTAction::Publish { retain, payload, .. } = action {
                assert!(*retain);
                assert!(!payload.is_empty());
            }
        }

        watcher.update_discovery(&device).await;
        assert!(drain(&mut rx).is_empty(), "second poll must not re-send discovery");
    }

    #[tokio::test]
    async fn stale_retained_configs_are_blanked_after_discovery() {
        let (mut watcher, mut rx) = FireboardWatcher::for_test(cfg("homeassistant"));
        let device = fixture();
        let hardware_id = device.hardware_id.clone();

        // Simulate the broker replaying its retained configs on subscribe.
        for channel in [0usize, 1, 2, 3, 4, 5, 6, 7, 164] {
            watcher
                .discovery
                .entry(hardware_id.clone())
                .or_default()
                .retained_in_broker
                .insert(channel);
        }
        watcher.update_discovery(&device).await;
        drain(&mut rx);

        watcher.cleanup_stale_channels(&device).await;
        let actions = drain(&mut rx);
        let mut topics = published_topics(&actions);
        topics.sort();
        assert_eq!(
            topics,
            vec![
                "fireboard2mqtt/GHK4F6CD2/channel_0/availability",
                "fireboard2mqtt/GHK4F6CD2/channel_164/availability",
                "fireboard2mqtt/GHK4F6CD2/channel_7/availability",
                "homeassistant/sensor/GHK4F6CD2/channel_0/config",
                "homeassistant/sensor/GHK4F6CD2/channel_164/config",
                "homeassistant/sensor/GHK4F6CD2/channel_7/config",
            ]
        );
        for action in &actions {
            if let MQTTAction::Publish { retain, payload, .. } = action {
                assert!(*retain, "deleting a retained topic needs the retain flag");
                assert!(payload.is_empty(), "deleting a retained topic needs an empty payload");
            }
        }

        // Cleaned channels are forgotten, so a second pass is a no-op.
        watcher.cleanup_stale_channels(&device).await;
        assert!(drain(&mut rx).is_empty());
        let state = &watcher.discovery[&hardware_id];
        let mut remaining: Vec<usize> = state.retained_in_broker.iter().copied().collect();
        remaining.sort_unstable();
        assert_eq!(remaining, vec![1, 2, 3, 4, 5, 6]);
    }

    #[tokio::test]
    async fn subscription_is_requested_once_per_device() {
        let (mut watcher, mut rx) = FireboardWatcher::for_test(cfg("homeassistant"));
        let hardware_id = "GHK4F6CD2".to_string();
        watcher.ensure_subscribed(&hardware_id).await;
        watcher.ensure_subscribed(&hardware_id).await;
        let actions = drain(&mut rx);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            MQTTAction::Subscribe { topic, .. } => {
                assert_eq!(topic, "homeassistant/sensor/GHK4F6CD2/+/config")
            }
            other => panic!("expected a subscribe, got {other:?}"),
        }
    }
}
