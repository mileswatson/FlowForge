use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use clap::Subcommand;
use flowforge::{network::config::NetworkConfig, rand::Rng};

#[derive(Subcommand, Clone, Debug)]
pub enum Algorithm {
    /// Train an instance of RemyCC
    Remy {
        /// Number of iterations to train for.
        #[arg(long, default_value_t = 10000)]
        iters: u32,
    },
}

pub fn train(config: &Path, _output: &Path, _algorithm: Algorithm) -> Result<()> {
    let file = File::open(config)?;
    let config: NetworkConfig =
        serde_json::from_reader(file).with_context(|| "Config had incorrect format!")?;
    let mut rng = Rng::from_seed(0);
    for _ in 0..10 {
        let network = rng.sample(&config);
        println!("{:?}", &network);
    }
    println!("{:?}", config);

    Ok(())
}
