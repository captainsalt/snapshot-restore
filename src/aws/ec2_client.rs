use crate::{AppConfig, cli_args::Args};
use aws_config::{Region, SdkConfig};

pub fn create_ec2_client(
    app_config: &AppConfig,
    args: &Args,
    aws_profile: &SdkConfig,
) -> aws_sdk_ec2::Client {
    let ec2_endpoint = app_config.get("EC2_ENDPOINT").cloned();
    let region = Region::new(args.region.to_string());

    // Use the custom HTTP client in your EC2 client configuration
    let ec2_config = aws_sdk_ec2::config::Builder::from(aws_profile)
        .region(Some(region))
        .set_endpoint_url(ec2_endpoint)
        .clone()
        .build();

    aws_sdk_ec2::client::Client::from_conf(ec2_config)
}
