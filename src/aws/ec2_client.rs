use aws_config::{BehaviorVersion, Region, SdkConfig, meta::region::RegionProviderChain};
use aws_sdk_ec2::Client as Ec2Client;
use hyper_rustls::ConfigBuilderExt;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, Error as TlsError, ServerName};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug)]
struct NoVerifyVerifier;

impl ServerCertVerifier for NoVerifyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: SystemTime,
    ) -> Result<ServerCertVerified, TlsError> {
        eprintln!(
            "WARNING: Skipping TLS certificate verification! This is insecure and should not be used in production."
        );
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &Certificate,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::HandshakeSignatureValid, TlsError> {
        eprintln!(
            "WARNING: Skipping TLS 1.2 signature verification! This is insecure and should not be used in production."
        );
        Ok(rustls::client::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &Certificate,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::HandshakeSignatureValid, TlsError> {
        eprintln!(
            "WARNING: Skipping TLS 1.3 signature verification! This is insecure and should not be used in production."
        );
        Ok(rustls::client::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub async fn build_aws_config(
    region: Option<String>,
    profile: Option<String>,
    skip_ssl_verify: bool,
) -> SdkConfig {
    let region_provider = RegionProviderChain::first_try(region.map(Region::new))
        .or_default_provider()
        .or_else(Region::new("us-east-1"));

    let mut config_loader = aws_config::defaults(BehaviorVersion::latest()).region(region_provider);

    if let Some(profile_name) = profile {
        config_loader = config_loader.profile_name(profile_name);
    }

    if skip_ssl_verify {
        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifyVerifier))
            .with_no_client_auth();

        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();

        let smithy_connector =
            aws_smithy_runtime::client::http::hyper_014::Adapter::builder().build(https_connector);

        config_loader = config_loader.http_connector(smithy_connector);
        eprintln!("AWS SDK Config: SSL verification DISABLED.");
    }

    config_loader.load().await
}

pub fn create_ec2_client(
    aws_sdk_config: &SdkConfig,
    custom_endpoint_url: Option<String>,
) -> Ec2Client {
    let mut ec2_config_builder = aws_sdk_ec2::config::Builder::from(aws_sdk_config);

    if let Some(endpoint) = custom_endpoint_url {
        eprintln!("Using custom EC2 endpoint: {}", endpoint);
        ec2_config_builder = ec2_config_builder.endpoint_url(endpoint);
    }

    let ec2_config = ec2_config_builder.build();

    Ec2Client::from_conf(ec2_config)
}
