use std::path::Path;

use anyhow::Result;
use flowforge::{
    flow::{FlowProperties, UtilityConfig},
    network::config::NetworkConfig,
    rand::Rng,
    time::Float,
    trainers::{delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, TrainerConfig},
    Config, Trainer,
};

pub fn _evaluate<T>(
    trainer_config: &T::Config,
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) -> (Float, FlowProperties)
where
    T: Trainer,
{
    let dna = T::Dna::load(input_path).unwrap();
    T::new(trainer_config)
        .evaluate(&dna, network_config, utility_config.inner(), rng)
        .unwrap()
}

pub fn evaluate(
    trainer_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
) -> Result<()> {
    let trainer_config = TrainerConfig::load(trainer_config)?;
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(534522);

    let (score, flow_properties) = match trainer_config {
        TrainerConfig::Remy(cfg) => {
            _evaluate::<RemyTrainer>(&cfg, &network_config, &utility_config, input_path, &mut rng)
        }
        TrainerConfig::DelayMultiplier(cfg) => _evaluate::<DelayMultiplierTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
    };

    println!(
        "Achieved expected utility {} with {}",
        score, flow_properties
    );

    Ok(())
}
