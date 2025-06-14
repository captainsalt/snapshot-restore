use clap::{Parser, command};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// AWS profile to use
    #[arg(long, short('p'))]
    pub profile: String,

    /// Path to file that contains instance names
    #[arg(long, short('f'))]
    pub instance_file: String,

    /// Looks for instances in specified region
    #[arg(long, short('r'))]
    pub region: String,

    /// Without this flag the application will not make any changes to the environment
    #[arg(long)]
    pub execute: bool,

    /// Start instances after restoring volume
    #[arg(long("start"), default_value_t = false, required(false))]
    pub start_instances: bool,

    /// Stop instances before restoring volume
    #[arg(long("stop"), default_value_t = false, required(false))]
    pub stop_instances: bool,
}
