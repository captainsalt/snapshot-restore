mod aws_authentication;
mod aws_ec2;
mod cli_args;
use aws_authentication::*;
use aws_config::SdkConfig;
use aws_ec2::{
    app_err::ApplicationError, find_instances_by_name, get_instance_snapshots,
    get_most_recent_snapshots,
};
use clap::Parser;
use cli_args::Args;
use config::Config;
use std::{collections::HashMap, fs};

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
    let ec2_endpoint = app_config.get("EC2_ENDPOINT").cloned();
    let ec2_config = aws_sdk_ec2::config::Builder::from(aws_profile)
        .set_endpoint_url(ec2_endpoint)
        .clone()
        .build();

    aws_sdk_ec2::client::Client::from_conf(ec2_config)
}

fn read_instance_names(input_file_path: &String) -> Result<Vec<String>, std::io::Error> {
    Ok(fs::read_to_string(input_file_path)?
        .lines()
        .map(String::from)
        .collect())
}

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    let args = Args::parse();
    let app_config = get_app_config();
    let aws_profile = get_profile(Some(&args.profile)).await;
    let ec2_client = create_ec2_client(&app_config, &aws_profile);

    let instance_names = read_instance_names(&args.instance_file)
        .map_err(|err| ApplicationError::from_err("Error reading instances from file", err))?;
    let instances = find_instances_by_name(&ec2_client, instance_names)
        .await
        .map_err(|err| ApplicationError::from_err("Could not find instances provided", err))?;

    let instance = instances.first().expect("Should be at least one instance");
    let snapshots = get_instance_snapshots(&ec2_client, instance).await;
    let recent_snapshots = get_most_recent_snapshots(instance, &snapshots.unwrap())
        .await
        .expect("Snapshots should exist");

    for snapshot in recent_snapshots {
        println!(
            "---
            Volume ID: {:?}
            Snapshot ID: {:?}
            ---",
            snapshot.volume_id(),
            snapshot.snapshot_id()
        )
    }

    Ok(())
}
