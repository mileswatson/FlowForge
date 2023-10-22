use std::{fs::File, marker::PhantomData, path::Path};

use anyhow::{Context, Result};
use clap::Subcommand;
use flowforge::{
    network::config::NetworkConfig,
    rand::Rng,
    trainers::remy::{RemyConfig, RemyTrainer},
    IgnoreResultTrainer, Trainer,
};

#[derive(Subcommand, Clone, Debug)]
pub enum Algorithm {
    /// Train an instance of RemyCC
    Remy {
        /// Number of iterations to train for.
        #[arg(long, default_value_t = 10000)]
        iters: u32,
    },
}

pub fn train(config: &Path, _output: &Path, algorithm: Algorithm) -> Result<()> {
    let file = File::open(config)?;
    let config: NetworkConfig =
        serde_json::from_reader(file).with_context(|| "Config had incorrect format!")?;
    let mut rng = Rng::from_seed(0);
    for _ in 0..10 {
        let network = rng.sample(&config);
        println!("{:?}", &network);
    }

    let trainer: Box<dyn Trainer<()>> = match algorithm {
        Algorithm::Remy { iters } => Box::new(IgnoreResultTrainer {
            trainer: RemyTrainer {},
            marker: PhantomData {},
        }),
    };

    println!("{:?}", config);

    Ok(())
}
