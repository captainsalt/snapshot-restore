mod aws_authentication;
mod aws_ec2;
mod cli;
use aws_authentication::*;
use aws_config::SdkConfig;
use aws_ec2::{find_instances_by_name, get_instance_snapshots, get_most_recent_snapshot};
use clap::Parser;
use cli::Args;
use config::Config;
use std::{collections::HashMap, fs};
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
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let app_config = get_app_config();
    let aws_profile = get_profile(Some(&args.profile)).await;
    let ec2_client = create_ec2_client(&app_config, &aws_profile);

    let instance_names = read_instance_names(&args.instance_file);
    let instances = find_instances_by_name(&ec2_client, instance_names.unwrap()).await;

    let instance = instances.first().unwrap();
    let snapshots = get_instance_snapshots(&ec2_client, instance).await;
    let snapshots = get_most_recent_snapshot(instance, &snapshots).await;

    for snapshot in snapshots {
        println!(
            "---
            Volume ID: {:?}
            Snapshot ID: {:?}
            ---",
            snapshot.volume_id(),
            snapshot.snapshot_id()
        )
    }

    // for snapshot in snapshots {
    //     print!(
    //         "---
    //         Completion time: {}
    //         Snapshot ID: {}
    //         Snapshot Name{}
    //         ---\n",
    //         snapshot
    //             .completion_time()
    //             .expect("Snapshot should have completion time"),
    //         snapshot.snapshot_id().expect("Snapshot ID should exist"),
    //         snapshot
    //             .tags()
    //             .iter()
    //             .find(|t| t.key().unwrap() == "Name")
    //             .unwrap()
    //             .value()
    //             .unwrap()
    //     );
    // }

    Ok(())
}
