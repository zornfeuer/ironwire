use super::*;
use tempfile::TempDir;
use std::sync::Arc;
use x509_parser::parse_x509_certificate;

#[test]
fn test_certificate_params_contains_required_sans() {
    let params = generate_dev_certificate_params()
        .expect("Unable to generate dev certificate params");
    
    assert!(params.subject_alt_names
        .contains(&SanType::DnsName(Ia5String::try_from("localhost").unwrap()))
    );
    assert!(params.subject_alt_names
        .contains(&SanType::IpAddress(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
    )));
    assert!(params.subject_alt_names
        .contains(&SanType::IpAddress(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
    )));
}

#[test]
fn test_generated_certificate_is_valid_and_parsable() {
    let params = generate_dev_certificate_params()
        .expect("Unable to generate dev certificate params");
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key_pair).unwrap();

    let der = cert.der();
    let (_rem, parsed) = parse_x509_certificate(der).expect("failed to parse generated cert");

    let sans: Vec<String> = parsed.extensions().iter()
        .find(|ext| ext.oid == x509_parser::oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME)
        .and_then(|ext| {
            use x509_parser::{extensions::{SubjectAlternativeName, GeneralName}, prelude::FromDer};
            SubjectAlternativeName::from_der(ext.value.as_ref())
                .ok()
                .map(|(_, san)| 
                    san.general_names
                        .iter()
                        .map(|gn| match gn {
                            GeneralName::DNSName(s) => format!("DNS:{}", s),
                            GeneralName::IPAddress(ip) => format!(
                                "IP:{}",
                                ip.iter()
                                .map(|&b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(".")
                            ),
                            _ => format!("{:?}", gn),
                        })
                        .collect()
                )
        })
        .unwrap_or_default();

    assert!(sans.contains(&"DNS:localhost".to_string()));
    assert!(sans.contains(&"IP:127.0.0.1".to_string()));
    assert!(sans.contains(&"IP:0.0.0.0".to_string()));
}

#[test]
fn test_load_rustls_config_returns_valid_config() {
    let temp = TempDir::new().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let config = load_rustls_config().expect("should generate dev config");

    assert_eq!(
        config.alpn_protocols,
        vec![b"h2".to_vec(), b"http/1.1".to_vec()],
        "ALPN must include HTTP/2 and HTTP/1.1 for WebSocket compatibility"
    );

    use rustls::server::ServerConnection;
    let _conn = ServerConnection::new(Arc::new(config))
        .expect("ServerConfig must be usable to create ServerConnection");
}
