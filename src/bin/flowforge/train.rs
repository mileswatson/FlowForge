use std::path::Path;

use anyhow::Result;
use flowforge::{
    network::config::NetworkConfig,
    rand::Rng,
    trainers::{
        delay_multiplier::{DelayMultiplierDna, DelayMultiplierTrainer},
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
        TrainerConfig::Remy(cfg) => {
            RemyTrainer::new(&cfg).train(
                &networks,
                &mut |progress, d: Option<&RemyDna>| {},
                &mut rng,
            );
        }
        TrainerConfig::DelayMultiplier(cfg) => {
            DelayMultiplierTrainer::new(&cfg).train(
                &networks,
                &mut |progress, d: Option<&DelayMultiplierDna>| {
                    if let Some(d) = d {
                        println!("{:?}", d);
                    }
                },
                &mut rng,
            );
        }
    };

    Ok(())
}
