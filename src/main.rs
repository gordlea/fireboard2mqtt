use crate::{
    config::load_cfg_from_env,
    fireboard_watcher::FireboardWatcher,
    mqtt_action::{MQTTAction, MQTTInbound},
};
use env_logger::{Builder, Env};
use human_bytes::human_bytes;
use log::{debug, error, info, trace, warn};
use memory_stats::memory_stats;
use rumqttc::v5::{AsyncClient, Event, Incoming, MqttOptions};
use std::process;
use tokio::{
    sync::mpsc,
    time::{self, sleep},
};


mod config;
mod constants;
mod device;
mod drive;
mod fireboard_api;
mod fireboard_watcher;
mod mqtt_action;
mod tls;
mod utils;


#[tokio::main]
async fn main() {
    let mut builder = Builder::from_env(Env::default());
    builder.target(env_logger::Target::Stdout);
    builder.init();

    
    let cfg = load_cfg_from_env();

    

    debug!("config loaded successfully: {}", serde_json::to_string_pretty(&cfg).unwrap());
    

    let (tx_mqtt, mut rx_mqtt) = mpsc::channel::<MQTTAction>(16);
    // Messages the broker sends us (retained discovery configs replayed on subscribe) flow the
    // other way, from the event loop below to the watcher.
    let (tx_inbound, rx_inbound) = mpsc::channel::<MQTTInbound>(256);
    // One rustls config for both the Fireboard API (reqwest) and the MQTT transport (rumqttc).
    // The MQTT CA file only applies when the broker url actually uses tls, matching the
    // "will be ignored" warning in config.rs.
    let tls = {
        let ca_file = if cfg.mqtt_use_tls { cfg.mqtt_tls_ca_file.as_deref() } else { None };
        match tls::build_client_config(ca_file) {
            Ok(tls) => tls,
            Err(e) => {
                error!("tls configuration error: {e:#}");
                process::exit(1);
            }
        }
    };

    let mut watcher = {
        let watcher_result = FireboardWatcher::new(&cfg, &tls, tx_mqtt.clone(), rx_inbound).await;
        if let Err(e) = watcher_result {
            error!("Error setting up FireboardWatcher: {:?}", e);
            process::exit(2);
        }
        watcher_result.unwrap()
    };

    let mqtt_use_tls = cfg.mqtt_use_tls;
    let (mqtt_client, mut mqtt_eventloop) = {
        let cfg = cfg.clone();
        let scheme = if cfg.mqtt_use_tls { "mqtts" } else { "mqtt" };
        let ca_note = match (cfg.mqtt_use_tls, &cfg.mqtt_tls_ca_file) {
            (true, Some(path)) => format!(" (tls, trusting system roots plus {path})"),
            (true, None) => " (tls, trusting system roots)".to_string(),
            (false, _) => String::new(),
        };
        info!(
            "connecting to mqtt broker at {}://{}:{} with clientId {}{}",
            scheme, cfg.mqtt_host, cfg.mqtt_port, cfg.mqtt_clientid, ca_note
        );
        let mut mqtt_options = MqttOptions::new(
            cfg.mqtt_clientid.clone(),
            cfg.mqtt_host.clone(),
            cfg.mqtt_port,
        );
        if cfg.mqtt_use_tls {
            mqtt_options.set_transport(tls::mqtt_transport(&tls));
        }
        // rumqttc sizes its outgoing-publish table from this limit and defaults to u16::MAX,
        // which preallocates ~13.5 MiB (measured; see examples/memprobe.rs). Only QoS 1/2
        // publishes occupy a slot and a full discovery burst is a few dozen, so 64 is plenty;
        // if it is ever reached rumqttc simply applies back-pressure until acks arrive.
        mqtt_options.set_outgoing_inflight_upper_limit(64);
        if let Some(mqtt_credentials) = cfg.mqtt_credentials {
            mqtt_options.set_credentials(mqtt_credentials.username, mqtt_credentials.password);
        }
        mqtt_options.set_last_will(watcher.get_last_will());
        AsyncClient::new(mqtt_options, 16)
    };

    tokio::spawn(async move {
        while let Some(action) = rx_mqtt.recv().await {
            match action {
                MQTTAction::Publish {
                    topic,
                    qos,
                    retain,
                    payload,
                    props,
                } => {
                    trace!("publishing to mqtt: topic={:?}, qos={:?}, retain={:?}, payload={:?}, props={:?}", topic, qos, retain, payload, props);
                    if payload.is_empty() {
                        // An empty retained payload is how a retained topic gets deleted; the
                        // watcher does this on purpose to clean up stale discovery configs.
                        debug!("publishing empty payload to topic: {}", topic)
                    }
                    if let Some(properties) = props {
                        mqtt_client
                            .publish_with_properties(topic, qos, retain, payload, properties)
                            .await
                            .unwrap();
                    } else {
                        mqtt_client
                            .publish(topic, qos, retain, payload)
                            .await
                            .unwrap();
                    }
                }
                MQTTAction::Subscribe { topic, qos, props } => {
                    if let Some(properties) = props {
                        mqtt_client
                            .subscribe_with_properties(topic, qos, properties)
                            .await
                            .unwrap();
                    } else {
                        mqtt_client.subscribe(topic, qos).await.unwrap();
                    }
                }
                MQTTAction::Unsubscribe { topic, props } => {
                    if let Some(properties) = props {
                        mqtt_client
                            .unsubscribe_with_properties(topic, properties)
                            .await
                            .unwrap();
                    } else {
                        mqtt_client.unsubscribe(topic).await.unwrap();
                    }
                }
            }
        }
    });

    tokio::spawn(async move {
        loop {
            watcher.update().await;
            if let Some(usage) = memory_stats() {
                info!(
                    "Current physical memory usage: {}",
                    human_bytes(usage.physical_mem as u32)
                );
            }
            debug!("there are {} devices online", watcher.online_device_count());
            let sleep_duration = if watcher.online_device_count() > 0 {
                cfg.fireboard_api_device_online_update_interval_seconds
            } else {
                // we default to polling once a minute when no devices are online
                cfg.fireboard_api_device_offline_update_interval_seconds
            };
            debug!(
                "updating from fireboard cloud api in {} seconds",
                sleep_duration
            );
            sleep(time::Duration::from_secs(sleep_duration)).await;
        }
    });


    loop {
        let event = mqtt_eventloop.poll().await;
        match &event {
            Ok(v) => {
                trace!("mqtt event: {v:?}");
                if let Event::Incoming(Incoming::Publish(publish)) = v {
                    let inbound = MQTTInbound {
                        topic: String::from_utf8_lossy(&publish.topic).into_owned(),
                        payload: publish.payload.clone(),
                        retain: publish.retain,
                    };
                    if let Err(e) = tx_inbound.try_send(inbound) {
                        warn!("dropping inbound mqtt message: {e}");
                    }
                }
            }
            Err(e) => {
                error!("mqtt error: {e}");
                if mqtt_use_tls {
                    error!(
                        "the mqtt connection uses tls: check that the broker is listening for tls on this port \
                         (use mqtt:// for a plaintext broker) and, for a self-signed or private ca, point \
                         FB2MQTT_MQTT_TLS_CA_FILE at its ca certificate"
                    );
                }
                process::exit(3);
            }
        }
    }
}
