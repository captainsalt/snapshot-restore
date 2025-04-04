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
}
