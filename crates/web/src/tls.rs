//! Optional native HTTPS. Used only when both `TLS_CERT_FILE` and `TLS_KEY_FILE`
//! are configured; otherwise the server speaks plain HTTP (e.g. behind a proxy).

use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Builds a rustls server config from a PEM certificate chain (leaf first) and a
/// PEM private key (PKCS#8, PKCS#1 or SEC1).
pub fn load_server_config(cert_file: &str, key_file: &str) -> Result<rustls::ServerConfig, String> {
    let certs = CertificateDer::pem_file_iter(cert_file)
        .map_err(|e| format!("cannot read TLS_CERT_FILE '{cert_file}': {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("invalid certificate in '{cert_file}': {e}"))?;
    if certs.is_empty() {
        return Err(format!("no certificate found in '{cert_file}'"));
    }
    let key = PrivateKeyDer::from_pem_file(key_file)
        .map_err(|e| format!("cannot read private key from TLS_KEY_FILE '{key_file}': {e}"))?;

    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("certificate and private key do not match: {e}"))
}

#[cfg(test)]
mod tests {
    use super::load_server_config;

    #[test]
    fn missing_files_are_reported() {
        let err = load_server_config("/nonexistent/cert.pem", "/nonexistent/key.pem").unwrap_err();
        assert!(err.contains("TLS_CERT_FILE"), "{err}");
    }
}
