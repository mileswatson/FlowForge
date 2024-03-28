use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use create_configs::create_all_configs;
use evaluate::evaluate;
use trace::trace;
use train::train;

mod create_configs;
mod evaluate;
mod trace;
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
        #[arg(long)]
        dna: PathBuf,

        /// File to write training progress to
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Evaluate a congestion control algorithm for a given network
    Evaluate {
        /// Trainer config file (JSON)
        #[arg(long)]
        trainer: PathBuf,

        /// Network config file (JSON)
        #[arg(long)]
        network: PathBuf,

        /// Utility function config file (JSON)
        #[arg(long)]
        utility: PathBuf,

        /// File to read congestion control algorithm DNA from
        #[arg(short, long)]
        input: PathBuf,
    },
    /// Trace the execution of a particular sender
    Trace {
        /// Flow mode
        #[arg(long)]
        mode: FlowAdders,

        /// Network config file (JSON)
        #[arg(long)]
        network: PathBuf,

        /// Utility function config file (JSON)
        #[arg(long)]
        utility: PathBuf,

        /// File to read congestion control algorithm DNA from
        #[arg(short, long)]
        input: PathBuf,

        /// File to output trace to (JSON)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Random seed to use
        #[arg(long, default_value_t = 12345)]
        seed: u64,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FlowAdders {
    Remy,
    Remyr,
    DelayMultiplier,
}

#[derive(Parser, Debug)]
#[command(author, version, about, about = "Use the FlowForge CLI to tailor congestion control algorithms to a provided network configuration.", long_about = None)]
struct Args {
    /// The maximum number of threads to use
    #[arg(short, long)]
    threads: Option<usize>,
    #[command(subcommand)]
    pub command: Command,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(threads) = args.threads {
        println!("Set number of threads to {}", threads);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .unwrap();
    }
    match args.command {
        Command::GenConfigs { output_folder } => create_all_configs(&output_folder),
        Command::Train {
            trainer,
            network,
            utility,
            dna,
            output,
        } => train(&trainer, &network, &utility, &dna, output.as_deref()),
        Command::Evaluate {
            trainer,
            network,
            utility,
            input,
        } => evaluate(&trainer, &network, &utility, &input),
        Command::Trace {
            mode,
            network,
            utility,
            input,
            output,
            seed,
        } => trace(&mode, &network, &utility, &input, output.as_deref(), seed),
    }
}
