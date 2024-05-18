use crate::{
    config::load_cfg_from_env, fireboard_watcher::FireboardWatcher, mqtt_action::MQTTAction,
};
use env_logger::{Builder, Env};
use human_bytes::human_bytes;
use log::{debug, error, info, trace, warn};
use memory_stats::memory_stats;
use rumqttc::v5::{AsyncClient, MqttOptions};
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
mod utils;


#[tokio::main]
async fn main() {
    let mut builder = Builder::from_env(Env::default());
    builder.target(env_logger::Target::Stdout);
    builder.init();

    
    let cfg = load_cfg_from_env();

    

    debug!("config loaded successfully: {}", serde_json::to_string_pretty(&cfg).unwrap());
    

    let (tx_mqtt, mut rx_mqtt) = mpsc::channel::<MQTTAction>(16);
    let mut watcher = {
        let watcher_result = FireboardWatcher::new(&cfg, tx_mqtt.clone()).await;
        if let Err(e) = watcher_result {
            error!("Error setting up FireboardWatcher: {:?}", e);
            process::exit(2);
        }
        watcher_result.unwrap()
    };

    let (mqtt_client, mut mqtt_eventloop) = {
        let cfg = cfg.clone();
        info!("connecting to mqtt broker at {}:{} with clientId {}", cfg.mqtt_host, cfg.mqtt_port, cfg.mqtt_clientid);
        let mut mqtt_options = MqttOptions::new(
            cfg.mqtt_clientid.clone(),
            cfg.mqtt_host.clone(),
            cfg.mqtt_port,
        );
        if let Some(mqtt_credentials) = cfg.mqtt_credentials {
            mqtt_options.set_credentials(mqtt_credentials.username, mqtt_credentials.password);
        }
        mqtt_options.set_last_will(watcher.get_last_will());
        AsyncClient::new(mqtt_options, 16)
    };

    tokio::spawn(async move {
        while let Some(action) = rx_mqtt.recv().await {
            // eprintln!("mqtt action: {:?}", action);

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
                        warn!("publishing empty payload to topic: {}", topic)
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
    // watcher.init().await;

    tokio::spawn(async move {
        loop {
            watcher.update().await;
            if let Some(usage) = memory_stats() {
                info!(
                    "Current physical memory usage: {}",
                    human_bytes(usage.physical_mem as u32)
                );
                // info!("Current virtual memory usage: {}", usage.virtual_mem);
            }
            debug!("there are {} devices online", watcher.online_device_count());
            let sleep_duration = if watcher.online_device_count() > 0 {
                // the fireboard cloud api has a rate limit of 200 requests per hour
                // which works out to 1 request every 18 seconds, or 1 every 20 secs to be safe,
                // so we need to be careful about how often we poll for updates
                let default_base_interval = 20;
                if cfg.fireboard_enable_drive {
                    debug!("drive support is enabled");
                    // if drive is enabled, then for each online device we need to make
                    // one extra call to the fireboard cloud api
                    default_base_interval * 2
                } else {
                    debug!("drive support not enabled");
                    // if drive is not enabled, then we only need to make one call total
                    // when we call `update()` to get the temps for all devices
                    default_base_interval
                }
            } else {
                // we default to polling once a minute when no devices are online
                60
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
            }
            Err(e) => {
                error!("mqtt error: {e:?}");
                process::exit(3);
            }
        }
    }
}
