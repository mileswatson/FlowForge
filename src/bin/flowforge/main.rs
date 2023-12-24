use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use create_configs::create_all_configs;
use train::train;

mod create_configs;
mod train;

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate all default config files (already in the /configs folder)
    GenConfigs {
        /// Folder to
        #[arg(short, long)]
        output_folder: PathBuf,
    },
    /// Tailor a congestion control algorithm for a given network
    Train {
        /// Trainer config file (JSON)
        #[arg(long)]
        trainer: PathBuf,

        /// Network config file (JSON)
        #[arg(long)]
        network: PathBuf,

        /// Utility function config file (JSON)
        #[arg(long)]
        utility: PathBuf,

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
    /* rayon::ThreadPoolBuilder::new()
    .num_threads(1)
    .build_global()
    .unwrap();*/
    let args = Args::parse();
    match args.command {
        Command::GenConfigs { output_folder } => create_all_configs(&output_folder),
        Command::Train {
            trainer,
            network,
            utility,
            output,
        } => train(&trainer, &network, &utility, &output),
    }
}
