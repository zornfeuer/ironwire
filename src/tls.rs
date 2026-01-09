use anyhow::{Context, Result};
use rustls::{
    ServerConfig,
    pki_types::PrivateKeyDer,
};
use rcgen::{
    CertificateParams,
    DistinguishedName,
    DnType,
    KeyPair,
    SanType,
    string::Ia5String
};

pub fn load_rustls_config() -> Result<ServerConfig> {
    if std::path::Path::new("cert.pem").exists() && std::path::Path::new("key.pem").exists() {
        return load_from_pem_files("cert.pem", "key.pem");
    }

    tracing::warn!("No cert.pem/key.pem found - generating dev certificate");
    generate_dev_certificate()
}

pub fn load_from_pem_files(cert_path: &str, key_path: &str) -> Result<ServerConfig> {
    let cert_bytes = std::fs::read(cert_path)?;
    let key_bytes = std::fs::read(key_path)?;

    let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(&cert_bytes[..]))
        .collect::<std::io::Result<Vec<_>>>()
        .context("failed to parse PEM certificates")?;

    let key = rustls_pemfile::private_key(&mut std::io::BufReader::new(&key_bytes[..]))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {}", key_path))?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(config)
}

pub fn generate_dev_certificate() -> Result<ServerConfig> {
    let params = generate_dev_certificate_params()?;

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    let cert_der = cert.der();
    let key_der = key_pair.serialize_der();

    let certs = vec![cert_der.clone()];
    let key = PrivateKeyDer::try_from(key_der)
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    std::fs::write("cert.pem", cert.pem())
        .map_err(|e| tracing::warn!("Failed to save cert.pem: {}", e)).ok();

    std::fs::write("key.pem", key_pair.serialize_pem())
        .map_err(|e| tracing::warn!("Failed to save key.pem: {}", e)).ok();

    Ok(config)
}

fn generate_dev_certificate_params() -> Result<CertificateParams> {
    let mut params = CertificateParams::new(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "0.0.0.0".to_string(),
    ])?;
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, "IronWire Dev CA");
    params.subject_alt_names = vec![
        SanType::DnsName(Ia5String::try_from("localhost")?),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
    ];
    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = time::OffsetDateTime::now_utc()
        .checked_add(time::Duration::days(365))
        .ok_or_else(|| {
            tracing::error!("Unable to make cert OffsetDateTime");
            anyhow::anyhow!("Unable to make cert OffsetDateTime")
        })?;

    Ok(params)
}

#[cfg(test)]
mod tests;
