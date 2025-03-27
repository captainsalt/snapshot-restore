use clap::{Parser, command};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, short)]
    pub profile: String,

    #[arg(long)]
    pub regions: Vec<String>,

    #[arg(long)]
    pub backup: bool,

    #[arg(long)]
    pub restore: bool,
}
