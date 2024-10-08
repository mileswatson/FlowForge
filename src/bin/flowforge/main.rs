use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use create_configs::create_all_configs;
use evaluate::evaluate;
use inspect::inspect;
use trace::trace;
use train::train;

mod create_configs;
mod evaluate;
mod inspect;
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
        #[arg(short, long)]
        config: PathBuf,

        /// Network config file (JSON)
        #[arg(long)]
        net: PathBuf,

        /// Utility function config file (JSON)
        #[arg(long)]
        util: PathBuf,

        /// File to write congestion control algorithm DNA to
        #[arg(long)]
        dna: PathBuf,

        /// OPTIONAL Run eval this number of times during training
        #[arg(long)]
        eval_times: Option<u32>,

        /// IF EVAL_EVERY Evaluation config file (JSON)
        #[arg(long)]
        eval: Option<PathBuf>,

        /// OPTIONAL, REQUIRES EVAL_EVERY File to write training progress to
        #[arg(long)]
        progress: Option<PathBuf>,

        /// OPTIONAL Force overwrite the DNA file if it exists
        #[arg(short, long)]
        force: bool,

        /// OPTIONAL Seed for training RNG
        #[arg(long, default_value_t = 5871837)]
        training_seed: u64,

        /// OPTIONAL Seed for evaluation RNG
        #[arg(long, default_value_t = 534522)]
        eval_seed: u64,
    },
    /// Evaluate a congestion control algorithm for a given network
    Evaluate {
        /// Evaluation config file (JSON)
        #[arg(short, long)]
        config: PathBuf,

        /// Flow mode
        #[arg(long)]
        mode: FlowAdders,

        /// Network config file (JSON)
        #[arg(long)]
        net: PathBuf,

        /// Utility function config file (JSON)
        #[arg(long)]
        util: PathBuf,

        /// File to read congestion control algorithm DNA from
        #[arg(short, long)]
        dna: PathBuf,

        /// OPTIONAL Seed for evaluation RNG
        #[arg(long, default_value_t = 534522)]
        eval_seed: u64,
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

        /// OPTIONAL File to output trace to (JSON)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// OPTIONAL Random seed
        #[arg(long, default_value_t = 534522)]
        seed: u64,
    },
    Inspect {
        /// Flow mode
        #[arg(long)]
        mode: FlowAdders,

        /// File to read congestion control algorithm DNA from
        #[arg(short, long)]
        dna: PathBuf,

        /// OPTIONAL File to output result to (JSON)
        #[arg(short, long)]
        output: Option<PathBuf>,
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
            config,
            eval,
            net,
            util,
            dna,
            progress,
            eval_times,
            force,
            training_seed,
            eval_seed,
        } => train(
            &config,
            &net,
            &util,
            &dna,
            eval_times,
            eval.as_deref(),
            progress.as_deref(),
            force,
            training_seed,
            eval_seed,
        ),
        Command::Evaluate {
            config,
            net,
            util,
            dna,
            mode,
            eval_seed,
        } => evaluate(&mode, &config, &net, &util, &dna, eval_seed),
        Command::Trace {
            mode,
            network,
            utility,
            input,
            output,
            seed,
        } => trace(&mode, &network, &utility, &input, output.as_deref(), seed),
        Command::Inspect { mode, dna, output } => {
            inspect(&dna, &mode, output.as_deref());
            Ok(())
        }
    }
}
