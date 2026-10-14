//! Builds the rustls transport used when the mqtt broker url has a tls scheme.
//!
//! rumqttc's `Transport::tls_with_default_config()` panics if the platform
//! certificate store cannot be read and offers no way to trust a private CA,
//! which is the common case for a self-signed Home Assistant Mosquitto add-on.
//! This module builds the root store by hand so both problems become ordinary
//! configuration errors.

use std::{fs, sync::Arc};

use anyhow::{bail, Context, Result};
use log::{debug, warn};
use rumqttc::tokio_rustls::rustls::{
    pki_types::{pem::PemObject, CertificateDer},
    ClientConfig, RootCertStore,
};
use rumqttc::{TlsConfiguration, Transport};

/// Builds a TLS transport that trusts the platform root certificates plus, when
/// given, every certificate in the PEM file at `ca_file`.
pub fn build_tls_transport(ca_file: Option<&str>) -> Result<Transport> {
    let mut root_store = RootCertStore::empty();

    let native = rustls_native_certs::load_native_certs();
    for err in &native.errors {
        warn!("problem loading a platform root certificate: {err}");
    }
    let (added, ignored) = root_store.add_parsable_certificates(native.certs);
    debug!("loaded {added} platform root certificates ({ignored} ignored)");

    if let Some(path) = ca_file {
        let pem = fs::read(path)
            .with_context(|| format!("could not read FB2MQTT_MQTT_TLS_CA_FILE {path}"))?;
        let certs = CertificateDer::pem_slice_iter(&pem)
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("could not parse PEM certificates in {path}"))?;
        if certs.is_empty() {
            bail!("no certificates found in FB2MQTT_MQTT_TLS_CA_FILE {path}");
        }
        let (added, ignored) = root_store.add_parsable_certificates(certs);
        if added == 0 {
            bail!("no usable CA certificates found in FB2MQTT_MQTT_TLS_CA_FILE {path} ({ignored} could not be parsed)");
        }
        debug!("loaded {added} CA certificates from {path} ({ignored} ignored)");
    }

    if root_store.is_empty() {
        bail!("no trusted root certificates available: the platform certificate store is empty and no FB2MQTT_MQTT_TLS_CA_FILE was given");
    }

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Transport::tls_with_config(TlsConfiguration::Rustls(Arc::new(client_config))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Transport` has no `Debug` impl, so `unwrap_err` is not available.
    fn expect_err(result: Result<Transport>) -> String {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(e) => format!("{e:#}"),
        }
    }

    fn scratch_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("fb2mqtt-tls-test-{}-{name}", std::process::id()));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn platform_roots_alone_are_enough() {
        // Requires a readable platform cert store, which every supported host and
        // both docker images (ca-certificates) provide.
        assert!(build_tls_transport(None).is_ok());
    }

    #[test]
    fn missing_ca_file_is_an_error_not_a_panic() {
        let err = expect_err(build_tls_transport(Some("/nonexistent/fb2mqtt-ca.pem")));
        assert!(err.contains("/nonexistent/fb2mqtt-ca.pem"), "{err}");
    }

    #[test]
    fn ca_file_without_certificates_is_an_error() {
        let path = scratch_file("empty.pem", b"this is not a certificate\n");
        let err = expect_err(build_tls_transport(path.to_str()));
        let _ = fs::remove_file(&path);
        assert!(err.contains("no certificates found"), "{err}");
    }

    #[test]
    fn ca_file_with_garbage_pem_is_an_error() {
        let path = scratch_file("garbage.pem", b"-----BEGIN CERTIFICATE-----\nnot base64!!\n-----END CERTIFICATE-----\n");
        let err = expect_err(build_tls_transport(path.to_str()));
        let _ = fs::remove_file(&path);
        assert!(err.contains("could not parse"), "{err}");
    }
}
