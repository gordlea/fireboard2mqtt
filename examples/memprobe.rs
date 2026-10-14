//! Brings up each part of the network stack in turn and prints the resident/heap
//! delta after each step, to attribute the process's memory without a profiler.
//!
//! Needs no Fireboard credentials: it only makes an anonymous GET to fireboard.io
//! and, if `.env` has an mqtt url, connects to that broker and waits for ConnAck.
//!
//!     cargo run --release --example memprobe

#[path = "../src/tls.rs"]
#[allow(dead_code)]
mod tls;

use std::{collections::HashMap, time::Duration};

use rumqttc::v5::{AsyncClient, Event, Incoming, MqttOptions};

fn mem() -> (f64, f64) {
    let mut rss = 0.0;
    let mut anon = 0.0;
    for line in std::fs::read_to_string("/proc/self/smaps_rollup").unwrap().lines() {
        let mut it = line.split_whitespace();
        match (it.next(), it.next()) {
            (Some("Rss:"), Some(v)) => rss = v.parse::<f64>().unwrap() / 1024.0,
            (Some("Anonymous:"), Some(v)) => anon = v.parse::<f64>().unwrap() / 1024.0,
            _ => {}
        }
    }
    (rss, anon)
}

struct Probe {
    last: (f64, f64),
}

impl Probe {
    fn new() -> Self {
        let m = mem();
        println!("{:<48} rss {:6.1} MiB  anon {:6.1} MiB", "start", m.0, m.1);
        Probe { last: m }
    }
    fn step(&mut self, label: &str) {
        let m = mem();
        println!(
            "{:<48} rss {:6.1} MiB  anon {:6.1} MiB   (anon {:+.2} MiB)",
            label,
            m.0,
            m.1,
            m.1 - self.last.1
        );
        self.last = m;
    }
}

fn dotenv() -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(s) = std::fs::read_to_string(".env") {
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                m.insert(k.to_string(), v.trim().trim_matches('"').to_string());
            }
        }
    }
    m
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut p = Probe::new();

    let provider = rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider();
    provider.install_default().ok();
    p.step("aws-lc-rs default provider installed");

    let tls = tls::build_client_config(None)?;
    p.step("rustls ClientConfig (system roots)");

    let addrs: Vec<_> = tokio::net::lookup_host("fireboard.io:443").await?.collect();
    p.step(&format!("dns lookup fireboard.io ({} addrs)", addrs.len()));

    {
        let _tcp = tokio::net::TcpStream::connect(addrs[0]).await?;
        p.step("tcp connect fireboard.io:443");
    }
    p.step("tcp dropped");

    let client = reqwest::Client::builder()
        .tls_backend_preconfigured(tls::ClientConfig::clone(&tls))
        .build()?;
    p.step("reqwest client built (preconfigured tls)");

    let resp = client.get("https://fireboard.io/").send().await?;
    let status = resp.status();
    let body = resp.bytes().await?;
    p.step(&format!("GET https://fireboard.io/ ({status}, {} B body)", body.len()));
    drop(body);

    let resp = client.get("https://fireboard.io/").send().await?;
    let body = resp.bytes().await?;
    drop(body);
    p.step("second GET (pooled connection)");

    drop(client);
    tokio::time::sleep(Duration::from_millis(200)).await;
    p.step("reqwest client dropped");

    let client2 = reqwest::Client::new();
    let resp = client2.get("https://fireboard.io/").send().await?;
    let _ = resp.bytes().await?;
    p.step("reqwest::Client::new() (platform verifier) + GET");
    drop(client2);
    tokio::time::sleep(Duration::from_millis(200)).await;
    p.step("second reqwest client dropped");

    let env = dotenv();
    if let Some(url) = env.get("FB2MQTT_MQTT_URL") {
        let url = url::Url::parse(url)?;
        let host = url.host_str().unwrap().to_string();
        let use_tls = matches!(url.scheme(), "mqtts" | "ssl" | "tls");
        let port = url.port().unwrap_or(if use_tls { 8883 } else { 1883 });
        let mut opts = MqttOptions::new("fb2mqtt-memprobe", host, port);
        if use_tls {
            opts.set_transport(tls::mqtt_transport(&tls));
        }
        if let Some(u) = env.get("FB2MQTT_MQTT_USERNAME") {
            opts.set_credentials(u, env.get("FB2MQTT_MQTT_PASSWORD").cloned().unwrap_or_default());
        }
        {
            let (_c, _e) = AsyncClient::new(opts.clone(), 16);
            p.step("rumqttc AsyncClient::new (default inflight = u16::MAX)");
        }
        p.step("dropped");
        opts.set_outgoing_inflight_upper_limit(64);
        let (client, mut eventloop) = AsyncClient::new(opts, 16);
        p.step("rumqttc AsyncClient::new (inflight limit 64)");
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::ConnAck(_))) => break,
                Ok(_) => {}
                Err(e) => {
                    println!("mqtt error: {e}");
                    break;
                }
            }
        }
        p.step("mqtt connected (ConnAck)");
        for _ in 0..3 {
            let _ = tokio::time::timeout(Duration::from_secs(2), eventloop.poll()).await;
        }
        p.step("mqtt idle 6s");
        client.disconnect().await.ok();
        drop(client);
        drop(eventloop);
        tokio::time::sleep(Duration::from_millis(200)).await;
        p.step("mqtt dropped");
    }

    drop(tls);
    p.step("tls config dropped");

    tokio::time::sleep(Duration::from_secs(2)).await;
    p.step("idle 2s");
    Ok(())
}
