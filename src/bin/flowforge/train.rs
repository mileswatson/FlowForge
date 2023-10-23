use std::path::Path;

use anyhow::Result;
use flowforge::{
    network::config::NetworkConfig,
    rand::Rng,
    trainers::{
        remy::{RemyDna, RemyTrainer},
        TrainerConfig,
    },
    Config, Trainer,
};

pub fn train(trainer_config: &Path, network_config: &Path, output: &Path) -> Result<()> {
    let trainer_config = TrainerConfig::load(trainer_config)?;
    let network_config = NetworkConfig::load(network_config)?;

    let mut rng = Rng::from_seed(0);

    let networks: Vec<_> = (0..100).map(|_| rng.sample(&network_config)).collect();

    match trainer_config {
        TrainerConfig::Remy(remy_config) => {
            RemyTrainer::new(&remy_config).train(&networks, &mut |_: &RemyDna| {});
        }
    };

    Ok(())
}
