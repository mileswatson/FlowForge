use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum ContinuousDistribution {
    Uniform { min: f32, max: f32 },
    Normal { mean: f32, std_dev: f32 },
}

#[derive(Serialize, Deserialize, Debug)]
struct NetworkConfig {
    rtt: ContinuousDistribution,
    throughput: ContinuousDistribution,
    loss_rate: ContinuousDistribution,
}

#[derive(Subcommand, Clone, Debug)]
enum Algorithm {
    /// Train an instance of RemyCC
    Remy {
        /// Number of iterations to train for.
        #[arg(long, default_value_t = 10000)]
        iters: u32,
    },
}

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
        Command::CreateNetworkConfig { .. } => todo!(),
        Command::Train { .. } => todo!(),
    }
}
