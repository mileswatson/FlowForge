use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use create_network_config::create_config;
use train::train;

mod create_network_config;
mod train;

#[derive(Subcommand, Debug, Clone)]
enum TrainerConfigCommand {
    /// Train an instance of RemyCC
    Remy,
    /// Train a DelayMultiplier agent using a genetic algorithm
    DelayMultiplier,
}

#[derive(Subcommand, Debug, Clone)]
enum ConfigCommand {
    /// Create a default network config
    Network,
    #[command(subcommand)]
    /// Create a trainer config
    Trainer(TrainerConfigCommand),
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a network or trainer config file
    GenConfig {
        #[command(subcommand)]
        config_type: ConfigCommand,

        /// File to write the network config to
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Tailor a congestion control algorithm for a given network
    Train {
        /// Trainer config file (JSON)
        #[arg(long)]
        trainer: PathBuf,

        /// Network config file (JSON)
        #[arg(long)]
        network: PathBuf,

        /// File to write congestion control algorithm DNA to
        #[arg(short, long)]
        output: PathBuf,
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
        Command::GenConfig {
            config_type,
            output,
        } => create_config(&config_type, &output),
        Command::Train {
            trainer,
            network,
            output,
        } => train(&trainer, &network, &output),
    }
}
