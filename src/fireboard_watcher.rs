//! # Fireboard Watcher
//! 
//! This module is responsible for watching the Fireboard API and updating the MQTT broker with the latest data
//! as changes occur. It also handles the MQTT discovery process for new devices and channels.
use chrono::Local;
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

    async fn update_discovery(&mut self, device: &FireboardApiDevice) {
        let hardware_id = device.hardware_id.clone();

        let parent_device = Some(MQTTDiscoveryDevice {
            configuration_url: Some(
                format!("https://fireboard.io/devices/{}/edit/", device.id).to_string(),
            ),
            connections: Some(vec![["mac".to_string(), device.device_log.mac_nic.clone()]]),
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
        });

        // set battery mqtt discovery
        let battery_id = format!("{}_battery", hardware_id);
        let battery_discovery = MQTTDiscoverySensor {
            unique_id: battery_id.clone(),
            object_id: battery_id,
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

        for channel in device.channels.clone() {
            // set channel mqtt discovery
            let channel_id = format!("{}_channel_{}", hardware_id, channel.channel);

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
            };
            self.tx
                .send(MQTTAction::Publish {
                    topic: self.get_topic_device_channel_discovery(&hardware_id, &channel.channel),
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
        let drive_id = format!("{}_drive", hardware_id);
        let drive_discovery = MQTTDiscoverySensor {
            unique_id: drive_id.clone(),
            object_id: drive_id.clone(),
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
            object_id: drive_mode_id.clone(),
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
            object_id: drive_setpoint_id.clone(),
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
            object_id: drive_lidpaused_id.clone(),
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

                    let batt_percentage = f32_to_u8_pct(device.device_log.v_batt_per);

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

                if device_online {
                    // do channel temperatures
                    for channel in device.channels {
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
                        .get_realtime_drivelog(device.uuid)
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
            }
        } else if let Err(err) = result {
            error!("Error fetching devices: {:?}", err);
        }
    }
}
