use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use create_network_config::create_network_config;
use train::{train, Algorithm};

mod create_network_config;
mod train;

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a network config file
    CreateNetworkConfig {
        /// File to write the network config to
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Tailor a congestion control algorithm for a given network
    Train {
        /// Network config JSON file
        #[arg(short, long)]
        config: PathBuf,

        /// File to write congestion control algorithm DNA to
        #[arg(short, long)]
        output: PathBuf,

        #[command(subcommand)]
        algorithm: Algorithm,
    },
}

#[derive(Parser, Debug)]
#[command(author, version, about, about = "Use the FlowForge CLI to tailor congestion control algorithms to a provided network configuration.", long_about = None)]
struct Args {
    /// Name of the person to greet
    #[command(subcommand)]
    pub command: Command,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::CreateNetworkConfig { output } => create_network_config(&output),
        Command::Train {
            config,
            output,
            algorithm,
        } => train(&config, &output, algorithm),
    }
}
