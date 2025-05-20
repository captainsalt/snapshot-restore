pub mod app_err;
mod aws;
mod cli_args;
mod tui;
use app_err::ApplicationError;
use aws::{
    authentication::get_profile,
    ec2_client::create_ec2_client,
    ec2_functions::{
        create_volumes_from_snapshots, find_instances_by_name, get_instance_snapshots,
        replace_volumes, start_instance, stop_instance,
    },
};
use aws_sdk_ec2::types::Instance;
use clap::Parser;
use cli_args::Args;
use config::Config;
use futures::future::join_all;
use std::{collections::HashMap, error::Error, fs};
use tui::pick_snapshots;

type AppConfig = HashMap<String, String>;

fn get_app_config() -> AppConfig {
    Config::builder()
        .add_source(config::File::with_name("settings.toml").required(true))
        .build()
        .unwrap()
        .try_deserialize::<HashMap<String, String>>()
        .unwrap()
}

fn read_instance_names(input_file_path: &String) -> Result<Vec<String>, std::io::Error> {
    Ok(fs::read_to_string(input_file_path)?
        .lines()
        .map(String::from)
        .collect())
}

fn instance_name(instance: &Instance) -> &str {
    instance
        .tags()
        .iter()
        .find(|tag| tag.key() == Some("Name"))
        .and_then(|tag| tag.value())
        .unwrap_or(instance.instance_id().unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let app_config = get_app_config();
    let aws_profile = get_profile(Some(args.profile.clone()), Some(args.region.clone())).await;
    let ec2_client = create_ec2_client(&app_config, &args, &aws_profile);

    let instance_names = read_instance_names(&args.instance_file)
        .map_err(|err| ApplicationError::from_err("Error reading instances from file", err))?;
    let instances = find_instances_by_name(&ec2_client, instance_names).await?;

    for instance in instances.iter() {
        let instance_id = instance.instance_id().unwrap().to_string();
        let snapshots = get_instance_snapshots(&ec2_client, &instance).await?;
        let selected_snapshots = pick_snapshots(&ec2_client, &instance, &snapshots).await?;

        if !args.execute {
            continue;
        }

        if args.stop_instances {
            println!("Stopping instance {}", instance_name(&instance));
            stop_instance(&ec2_client, &instance_id).await?;
        }

        let volumes = create_volumes_from_snapshots(&ec2_client, &selected_snapshots).await?;
        replace_volumes(&ec2_client, &instance, &volumes).await?;
    }

    if args.start_instances {
        let start_futures = instances
            .iter()
            .map(|i| start_instance(&ec2_client, i.instance_id().unwrap()));

        join_all(start_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<()>, ApplicationError>>()?;
    }

    Ok(())
}
