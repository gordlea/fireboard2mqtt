use std::sync::{ atomic::AtomicBool, Arc };
use rumqttc::v5::{ AsyncClient, MqttOptions };
use memory_stats::memory_stats;
use log::{ debug, error, info, trace };
use tokio::{ sync::mpsc, time::{ self, sleep } };
use crate::{
    config::load_cfg_from_env,
    fireboard_watcher::FireboardWatcher,
    mqtt_action::MQTTAction,
};
use human_bytes::human_bytes;

mod config;
mod device;
mod drive;
mod fireboard_watcher;
mod mqtt_action;
mod fireboard_api;

pub const ONLINE: &str = "online";
pub const OFFLINE: &str = "offline";

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    pretty_env_logger::init();
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;

    let cfg = load_cfg_from_env();

    // let env_config = load_cfg_from_env();

    debug!("config loaded successfully");

    let (tx_mqtt, mut rx_mqtt) = mpsc::channel::<MQTTAction>(16);
    let mut watcher = FireboardWatcher::new(&cfg, tx_mqtt.clone()).await.unwrap();

    let (mqtt_client, mut mqtt_eventloop) = {
        let cfg = cfg.clone();
        let mut mqtt_options = MqttOptions::new(
            cfg.mqtt_clientid.clone(),
            cfg.mqtt_host.clone(),
            cfg.mqtt_port.clone()
        );
        if let Some((username, password)) = cfg.mqtt_credentials {
            mqtt_options.set_credentials(username, password);
        }
        mqtt_options.set_last_will(watcher.get_last_will());
        AsyncClient::new(mqtt_options, 16)
    };

    tokio::spawn(async move {
        while let Some(action) = rx_mqtt.recv().await {
            match action {
                MQTTAction::Publish { topic, qos, retain, payload, props } => {
                    if let Some(properties) = props {
                        mqtt_client
                            .publish_with_properties(topic, qos, retain, payload, properties).await
                            .unwrap();
                    } else {
                        mqtt_client.publish(topic, qos, retain, payload).await.unwrap();
                    }
                }
                MQTTAction::Subscribe { topic, qos, props } => {
                    if let Some(properties) = props {
                        mqtt_client
                            .subscribe_with_properties(topic, qos, properties).await
                            .unwrap();
                    } else {
                        mqtt_client.subscribe(topic, qos).await.unwrap();
                    }
                }
                MQTTAction::Unsubscribe { topic, props } => {
                    if let Some(properties) = props {
                        mqtt_client.unsubscribe_with_properties(topic, properties).await.unwrap();
                    } else {
                        mqtt_client.unsubscribe(topic).await.unwrap();
                    }
                }
            }
        }
    });
    // watcher.init().await;

    tokio::spawn(async move {
        let fb_update_loop_done = false;
        // let mut sleep_duration = time::Duration::from_secs(30);
        while !fb_update_loop_done {
            // let mut update_interval = time::interval(time::Duration::from_secs(30));
            // update_interval.tick().await;
            watcher.update().await;
            if let Some(usage) = memory_stats() {
                info!("Current physical memory usage: {}", human_bytes(usage.physical_mem as u32));
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
                    default_base_interval * (2 * 1)
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
            debug!("updating from fireboard cloud api in {} seconds", sleep_duration);
            sleep(time::Duration::from_secs(sleep_duration)).await;
        }
    });

    let done = false;
    while !done {
        let event = mqtt_eventloop.poll().await;
        match &event {
            Ok(v) => {
                trace!("mqtt Event = {v:?}");
            }
            Err(e) => {
                error!("Error = {e:?}");
                return Ok(());
            }
        }
    }
    Ok(())
}
