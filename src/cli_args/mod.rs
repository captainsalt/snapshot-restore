use clap::{Parser, command};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, short('p'))]
    pub profile: String,

    #[arg(long, short('f'))]
    pub instance_file: String,

    #[arg(long, short('r'))]
    pub region: String,

    #[arg(long, short('r'), default_value_t = true)]
    pub dry_run: bool,

    #[arg(long("start"), default_value_t = false)]
    pub start_instances: bool,

    #[arg(long("stop"), default_value_t = false)]
    pub stop_instances: bool,
}
