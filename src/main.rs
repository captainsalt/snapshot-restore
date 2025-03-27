mod cli;
use aws_authentication::*;
use clap::Parser;
use cli::Args;
use tokio::time::error::Error;
mod aws_authentication;
mod aws_ec2;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let profile = get_config(Some(&args.profile)).await;

    Ok(())
}
