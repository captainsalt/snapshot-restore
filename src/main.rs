#[allow(unused)]
mod cli;
use aws_authentication::*;
use aws_ec2::find_instances_by_name;
use aws_sdk_ec2::client;
use clap::Parser;
use cli::Args;
use tokio::time::error::Error;
mod aws_authentication;
mod aws_ec2;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let config = get_config(Some(&args.profile)).await;

    let ec2_client = client::Client::new(&config);
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
