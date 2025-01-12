//! # Fireboard Watcher
//! 
//! This module is responsible for watching the Fireboard API and updating the MQTT broker with the latest data
//! as changes occur. It also handles the MQTT discovery process for new devices and channels.
use chrono::Local;
use compact_str::{format_compact, CompactString};
use rumqttc::v5::mqttbytes::v5::LastWill;
use rumqttc::v5::mqttbytes::QoS;
use tokio::sync::mpsc::Sender;

use anyhow::Result;

use log::{debug, error, info, trace};

use crate::config::Fb2MqttConfig;
use crate::constants::{FIREBOARD_DEVICELOG_UPDATE_INTERVAL_MINUTES, OFF, OFFLINE, ON, ONLINE};
use crate::device::{
    MQTTDiscoveryAvailabilityEntry, MQTTDiscoveryBinarySensor, MQTTDiscoveryDevice,
    MQTTDiscoverySensor,
};
use crate::drive::DriveAttributes;
use crate::fireboard_api::{DriveModeType, FireboardApiClient, FireboardApiDevice};
use crate::mqtt_action::MQTTAction;
use crate::utils::f32_to_u8_pct;


pub struct FireboardWatcher {
    online_device_count: u8,
    fb_client: FireboardApiClient,
    tx: Sender<MQTTAction>,
    cfg: Fb2MqttConfig,
}

impl FireboardWatcher {
    pub async fn new(cfg: &Fb2MqttConfig, tx: Sender<MQTTAction>) -> Result<FireboardWatcher> {
        let fb_client = FireboardApiClient::new(
            cfg.fireboardaccount_email.clone(),
            cfg.fireboardaccount_password.clone(),
        )
        .await?;
        debug!("client authenticated successfully");

        let mut fb_watcher = FireboardWatcher {
            online_device_count: 0,
            fb_client,
            tx,
            cfg: cfg.clone(),
        };
        fb_watcher.init().await;
        Ok(fb_watcher)
    }

    pub fn online_device_count(&self) -> u8 {
        self.online_device_count
    }

    pub fn get_topic_bridge_availablility(&self) -> CompactString {
        format_compact!("{}/bridge/availability", self.cfg.mqtt_base_topic)
    }

    pub fn get_discovery_sensor_base_topic(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/sensor/{}",
            self.cfg.mqtt_discovery_topic, device_identifier
        )
    }

    pub fn get_discovery_binary_sensor_base_topic(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/binary_sensor/{}",
            self.cfg.mqtt_discovery_topic, device_identifier
        )
    }

    pub fn get_device_base_topic(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!("{}/{}", self.cfg.mqtt_base_topic, device_identifier)
    }

    pub fn get_topic_device_availablility(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/availability",
            self.get_device_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_battery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!("{}/battery", self.get_device_base_topic(device_identifier))
    }

    pub fn get_topic_device_battery_discovery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/battery/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_channel(&self, device_identifier: &CompactString, channel: &usize) -> CompactString {
        format_compact!(
            "{}/channel_{}",
            self.get_device_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_channel_availability(
        &self,
        device_identifier: &CompactString,
        channel: &usize,
    ) -> CompactString {
        format_compact!(
            "{}/channel_{}/availability",
            self.get_device_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_channel_discovery(
        &self,
        device_identifier: &CompactString,
        channel: &usize,
    ) -> CompactString {
        format_compact!(
            "{}/channel_{}/config",
            self.get_discovery_sensor_base_topic(device_identifier),
            channel
        )
    }

    pub fn get_topic_device_drive_discovery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/drive/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drivemode_discovery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/drivemode/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive_setpoint_discovery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/drive_setpoint/config",
            self.get_discovery_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive_lidpaused_discovery(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/drive_lidpaused/config",
            self.get_discovery_binary_sensor_base_topic(device_identifier)
        )
    }

    pub fn get_topic_device_drive(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!("{}/drive", self.get_device_base_topic(device_identifier))
    }

    pub fn get_topic_device_drive_availability(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/availability",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_setpoint_availability(
        &self,
        device_identifier: &CompactString,
    ) -> CompactString {
        format_compact!(
            "{}/setpoint_availability",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_state(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!("{}/state", self.get_topic_device_drive(device_identifier))
    }

    pub fn get_topic_device_drive_mode(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!("{}/mode", self.get_topic_device_drive(device_identifier))
    }

    pub fn get_topic_device_drive_setpoint(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/setpoint",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_lidpaused(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/lidpaused",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_topic_device_drive_attributes(&self, device_identifier: &CompactString) -> CompactString {
        format_compact!(
            "{}/attributes",
            self.get_topic_device_drive(device_identifier)
        )
    }

    pub fn get_last_will(&self) -> LastWill {
        let topic = self.get_topic_bridge_availablility();
        let topic_bytes = topic.to_string();
        LastWill {
            topic: topic_bytes.into(),
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
                topic: self.get_topic_bridge_availablility().to_string(),
                qos: QoS::AtLeastOnce,
                retain: true,
                payload: ONLINE.into(),
                props: None,
            })
            .await
            .unwrap();
    }

    async fn update_discovery(&mut self, device: &FireboardApiDevice) {
        let hardware_id = device.hardware_id.clone();

        let parent_device = Some(MQTTDiscoveryDevice {
            configuration_url: Some(
                format_compact!("https://fireboard.io/devices/{}/edit/", device.id),
            ),
            connections: Some(vec![[CompactString::const_new("mac"), device.device_log.mac_nic.clone()]]),
            identifiers: Some(vec![
                CompactString::from(device.id.to_string()),
                device.hardware_id.clone(),
                device.uuid.clone(),
            ]),
            manufacturer: Some(CompactString::from("Fireboard Labs")),
            model: Some(device.model.clone()),
            name: Some(device.title.clone()),
            serial_number: Some(device.hardware_id.clone()),
            sw_version: Some(device.version.clone()),
            ..MQTTDiscoveryDevice::default()
        });

        // set battery mqtt discovery
        let battery_id = format_compact!("{}_battery", hardware_id);
        let battery_discovery = MQTTDiscoverySensor {
            unique_id: battery_id.clone(),
            object_id: battery_id,
            name: Some(CompactString::from("Battery")),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
            ],
            device_class: Some(CompactString::from("battery")),
            qos: 0,
            icon: None,
            state_topic: self.get_topic_device_battery(&hardware_id),
            unit_of_measurement: Some(CompactString::from("%")),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_battery_discovery(&hardware_id).to_string(),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: battery_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        for channel in device.channels.clone() {
            // set channel mqtt discovery
            let channel_id = format_compact!("{}_channel_{}", hardware_id, channel.channel);

            let channel_topic = self.get_topic_device_channel(&hardware_id, &channel.channel);

            let channel_discovery = MQTTDiscoverySensor {
                unique_id: channel_id.clone(),
                object_id: channel_id,
                name: Some(channel.channel_label),
                availability: vec![
                    MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                    MQTTDiscoveryAvailabilityEntry::from(
                        self.get_topic_device_availablility(&hardware_id),
                    ),
                    MQTTDiscoveryAvailabilityEntry::from(
                        self.get_topic_device_channel_availability(&hardware_id, &channel.channel),
                    ),
                ],
                suggested_unit_of_measurement: Some(CompactString::from("°F")),
                device_class: Some(CompactString::from("temperature")),
                qos: 0,
                icon: None,
                state_topic: format_compact!("{}/state", channel_topic),
                unit_of_measurement: Some(CompactString::from(device.degreetype.to_string())),
                device: parent_device.clone(),
                // TODO make this configurable?
                expires_after: Some(600),
                ..MQTTDiscoverySensor::default()
            };
            self.tx
                .send(MQTTAction::Publish {
                    topic: self.get_topic_device_channel_discovery(&hardware_id, &channel.channel).to_string(),
                    qos: QoS::AtMostOnce,
                    retain: true,
                    payload: channel_discovery.into(),
                    props: None,
                })
                .await
                .unwrap();
        }

        // if drive_enabled {
        // set drive mqtt discovery
        let drive_id = format_compact!("{}_drive", hardware_id);
        let drive_discovery = MQTTDiscoverySensor {
            unique_id: drive_id.clone(),
            object_id: drive_id.clone(),
            name: Some(CompactString::from("Drive")),
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
            icon: Some(CompactString::from("mdi:fan")),
            // TODO make this configurable?
            expires_after: Some(600),
            state_topic: self.get_topic_device_drive_state(&hardware_id),
            unit_of_measurement: Some(CompactString::from("%")),
            device: parent_device.clone(),
            json_attributes_topic: Some(self.get_topic_device_drive_attributes(&hardware_id)),
            ..MQTTDiscoverySensor::default()
        };

        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drive_discovery(&hardware_id).to_string(),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_mode_id = format_compact!("{}_mode", drive_id.clone());
        let drive_mode_discovery = MQTTDiscoverySensor {
            unique_id: drive_mode_id.clone(),
            object_id: drive_mode_id.clone(),
            name: Some(CompactString::from("Drive Mode")),
            availability: vec![
                MQTTDiscoveryAvailabilityEntry::from(self.get_topic_bridge_availablility()),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_availablility(&hardware_id),
                ),
                MQTTDiscoveryAvailabilityEntry::from(
                    self.get_topic_device_drive_availability(&hardware_id),
                ),
            ],
            device_class: Some(CompactString::from("enum")),
            options: Some(vec![
                CompactString::from("off"),
                CompactString::from("manual"),
                CompactString::from("auto"),
            ]),
            qos: 0,
            icon: Some(CompactString::from("mdi:fan-alert")),
            state_class: None,
            // icon: None,
            state_topic: self.get_topic_device_drive_mode(&hardware_id),
            // unit_of_measurement: Some("%".to_string()),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drivemode_discovery(&hardware_id).to_string(),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_mode_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_setpoint_id = format_compact!("{}_setpoint", drive_id.clone());
        let drive_setpoint_discovery = MQTTDiscoverySensor {
            unique_id: drive_setpoint_id.clone(),
            object_id: drive_setpoint_id.clone(),
            name: Some("Drive Setpoint".into()),
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
            icon: Some(CompactString::from("mdi:thermometer-auto")),
            suggested_unit_of_measurement: Some(CompactString::from("°F")),
            device_class: Some(CompactString::from("temperature")),
            qos: 0,
            state_topic: self.get_topic_device_drive_setpoint(&hardware_id),
            unit_of_measurement: Some(device.degreetype.into()),
            device: parent_device.clone(),
            ..MQTTDiscoverySensor::default()
        };
        self.tx
            .send(MQTTAction::Publish {
                topic: self.get_topic_device_drive_setpoint_discovery(&hardware_id).to_string(),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_setpoint_discovery.into(),
                props: None,
            })
            .await
            .unwrap();

        let drive_lidpaused_id = format_compact!("{}_lidpaused", drive_id.clone());
        let drive_lidpaused_discovery = MQTTDiscoveryBinarySensor {
            unique_id: drive_lidpaused_id.clone(),
            object_id: drive_lidpaused_id.clone(),
            name: Some(CompactString::from("Drive Lid Paused")),
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
                topic: self.get_topic_device_drive_lidpaused_discovery(&hardware_id).to_string(),
                qos: QoS::AtMostOnce,
                retain: true,
                payload: drive_lidpaused_discovery.into(),
                props: None,
            })
            .await
            .unwrap();
    }

    pub async fn update(&mut self) {
        info!("checking fireboard api for updates");
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
            trace!("devices fetched successfully: {:?}", &returned_devices);

            self.online_device_count = 0;

            for device in returned_devices {
                let hardware_id = device.hardware_id.clone();

                debug!("found device: {:?}", hardware_id);

                let latest_temps = device.latest_temps.clone();
                let device_online = {
                    let has_latest_temps = !latest_temps.is_empty();
                    if has_latest_temps {
                        true
                    } else {
                        let now = Local::now();
                        let diff = now - device.device_log.date;
                        diff.num_minutes() < FIREBOARD_DEVICELOG_UPDATE_INTERVAL_MINUTES
                    }
                };

                // set device availability
                self.tx
                    .send(MQTTAction::Publish {
                        topic: self.get_topic_device_availablility(&hardware_id).to_string(),
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

                    let batt_percentage = f32_to_u8_pct(device.device_log.v_batt_per);
                    let payload_str = format_compact!("{batt_percentage}").to_string();
                    self.tx
                        .send(MQTTAction::Publish {
                            topic: self.get_topic_device_battery(&hardware_id).to_string(),
                            qos: QoS::AtMostOnce,
                            retain: true,
                            payload: payload_str.into(),
                            props: None,
                        })
                        .await
                        .unwrap();
                }

                if device_online {
                    // do channel temperatures
                    for channel in device.channels {
                        // let unique_id = format_compact!("{}_{}", device.hardware_id.clone(), channel.channel);
                        let channel_topic =
                            self.get_topic_device_channel(&hardware_id, &channel.channel);

                        // channel availability
                        self.tx
                            .send(MQTTAction::Publish {
                                topic: format!("{}/availability", channel_topic).to_string(),
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
                                    topic: format!("{}/state", channel_topic).to_string(),
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
                            //         topic: format_compact!("{}/state", channel_topic),
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
                        .get_realtime_drivelog(device.uuid)
                        .await;
                    if let Ok(rt_drivelog) = rt_drivelog_request {
                        if let Some(drivelog) = &rt_drivelog {
                            // drive not available
                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_availability(&hardware_id).to_string(),
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
                                    topic: self.get_topic_device_drive_state(&hardware_id).to_string(),
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
                                        topic: self.get_topic_device_drive_setpoint(&hardware_id).to_string(),
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
                                        ).to_string(),
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
                                        topic: self.get_topic_device_drive_setpoint(&hardware_id).to_string(),
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
                                        ).to_string(),
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
                                    topic: self.get_topic_device_drive_lidpaused(&hardware_id).to_string(),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: if drivelog.lidpaused { ON } else { OFF }.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_attributes(&hardware_id).to_string(),
                                    qos: QoS::AtMostOnce,
                                    retain: false,
                                    payload: drive_attributes.into(),
                                    props: None,
                                })
                                .await
                                .unwrap();

                            self.tx
                                .send(MQTTAction::Publish {
                                    topic: self.get_topic_device_drive_mode(&hardware_id).to_string(),
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
                                    topic: self.get_topic_device_drive_availability(&hardware_id).to_string(),
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
                            topic: self.get_topic_device_drive_availability(&hardware_id).to_string(),
                            qos: QoS::AtMostOnce,
                            retain: true,
                            payload: OFFLINE.into(),
                            props: None,
                        })
                        .await
                        .unwrap();
                }
            }
        } else if let Err(err) = result {
            error!("Error fetching devices: {:?}", err);
        }
    }
}
