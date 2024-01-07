use std::path::Path;

use anyhow::Result;
use flowforge::{
    flow::UtilityConfig,
    network::config::NetworkConfig,
    rand::Rng,
    trainers::{delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, TrainerConfig},
    Config, Trainer,
};

pub fn _train<T>(
    trainer_config: &T::Config,
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    output_path: &Path,
    rng: &mut Rng,
) where
    T: Trainer,
{
    assert!(T::Dna::valid_path(output_path));
    T::new(trainer_config)
        .train(
            network_config,
            utility_config.inner(),
            &mut |_progress, d: Option<&T::Dna>| {
                if let Some(x) = d {
                    x.save(output_path).unwrap()
                }
            },
            rng,
        )
        .save(output_path)
        .unwrap();
}

pub fn train(
    trainer_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    output: &Path,
) -> Result<()> {
    let trainer_config = TrainerConfig::load(trainer_config)?;
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(0);

    match trainer_config {
        TrainerConfig::Remy(cfg) => {
            _train::<RemyTrainer>(&cfg, &network_config, &utility_config, output, &mut rng)
        }
        TrainerConfig::DelayMultiplier(cfg) => _train::<DelayMultiplierTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            output,
            &mut rng,
        ),
    };

    Ok(())
}
