use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use clap::Subcommand;
use flowforge::network::config::NetworkConfig;

#[derive(Subcommand, Clone, Debug)]
pub enum Algorithm {
    /// Train an instance of RemyCC
    Remy {
        /// Number of iterations to train for.
        #[arg(long, default_value_t = 10000)]
        iters: u32,
    },
}

pub fn train(config: &Path, output: &Path, algorithm: Algorithm) -> Result<()> {
    let file = File::open(config)?;
    let config: NetworkConfig =
        serde_json::from_reader(file).with_context(|| "Config had incorrect format!")?;
    println!("{:?}", config);

    Ok(())
}
