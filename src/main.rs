#[allow(unused)]
mod aws_authentication;
mod aws_ec2;
mod cli;
use aws_authentication::*;
use aws_config::SdkConfig;
use aws_ec2::find_instances_by_name;
use aws_sdk_ec2::client;
use clap::Parser;
use cli::Args;
use config::Config;
use std::collections::HashMap;
use tokio::time::error::Error;

type AppConfig = HashMap<String, String>;

fn get_app_config() -> AppConfig {
    Config::builder()
        .add_source(config::File::with_name("settings.toml").required(true))
        .build()
        .unwrap()
        .try_deserialize::<HashMap<String, String>>()
        .unwrap()
}

fn create_ec2_client(app_config: &AppConfig, aws_profile: &SdkConfig) -> aws_sdk_ec2::Client {
    let ec2_config = aws_sdk_ec2::config::Builder::from(aws_profile)
        .endpoint_url(
            app_config
                .get("EC2_ENDPOINT")
                .expect("No endpoint provided"),
        )
        .build();

    client::Client::from_conf(ec2_config)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let app_config = get_app_config();
    let aws_profile = get_profile(Some(&args.profile)).await;
    let ec2_client = create_ec2_client(&app_config, &aws_profile);
    let instances = find_instances_by_name(&ec2_client, vec!["dw-instance-0"]).await;

    let instance_name = instances
        .first()
        .and_then(|instance| {
            instance.tags.as_ref().and_then(|tags| {
                tags.iter()
                    .find(|t| t.key.as_deref().unwrap_or_default() == "Name")
            })
        })
        .and_then(|name_tag| name_tag.value.clone())
        .unwrap_or_else(|| "No name found".to_string());

    println!("Found instance: {}", instance_name);

    Ok(())
}
