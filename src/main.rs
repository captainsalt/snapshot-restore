pub mod app_err;
mod aws;
mod cli_args;
mod tui;
use app_err::ApplicationError;
use aws::{
    authentication::get_profile,
    ec2_client::create_ec2_client,
    ec2_functions::{
        attach_new_volumes, create_volumes_from_snapshots, find_instances_by_name,
        get_instance_snapshots, start_instance, stop_instance,
    },
};
use clap::Parser;
use cli_args::Args;
use config::Config;
use std::{collections::HashMap, fs};
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

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    let args = Args::parse();
    let app_config = get_app_config();
    let aws_profile = get_profile(Some(&args.profile)).await;
    let ec2_client = create_ec2_client(&app_config, &args, &aws_profile);

    let instance_names = read_instance_names(&args.instance_file)
        .map_err(|err| ApplicationError::from_err("Error reading instances from file", err))?;
    let instances = find_instances_by_name(&ec2_client, instance_names).await?;

    for instance in instances {
        let instance_id = instance.instance_id().unwrap().to_string();
        let snapshots = get_instance_snapshots(&ec2_client, &instance).await?;
        let selected_snapshots = pick_snapshots(&ec2_client, &instance, &snapshots).await?;

        if !args.dry_run {
            if args.stop_instances {
                stop_instance(&ec2_client, &instance_id).await?;
            }

            let volumes = create_volumes_from_snapshots(&ec2_client, &selected_snapshots).await?;
            attach_new_volumes(&ec2_client, &instance, &volumes).await?;

            if args.start_instances {
                start_instance(&ec2_client, &instance_id).await?;
            }
        }
    }

    Ok(())
}
