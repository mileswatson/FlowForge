use std::path::Path;

use anyhow::Result;
use flowforge::{
    core::rand::Rng,
    flow::UtilityConfig,
    network::config::NetworkConfig,
    trainers::{
        delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer,
        TrainerConfig,
    },
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
    let starting_point = T::Dna::load(output_path).ok().and_then(|d| loop {
        let mut buf = String::new();
        println!("There is already valid DNA in the output path. Would you like to use it as a starting point? Y/N");
        std::io::stdin().read_line(&mut buf).unwrap();
        if buf.to_lowercase().trim() == "y" {
            return Some(d)
        } else if buf.to_lowercase().trim() == "n" { 
            return None
        }
    });
    T::new(trainer_config)
        .train(
            starting_point,
            network_config,
            utility_config,
            &mut |progress, d: Option<&T::Dna>| {
                if let Some(x) = d {
                    x.save(output_path).unwrap();
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
    output_path: &Path,
) -> Result<()> {
    let trainer_config = TrainerConfig::load(trainer_config)?;
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(534522);

    match trainer_config {
        TrainerConfig::Remy(cfg) => _train::<RemyTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            output_path,
            &mut rng,
        ),
        TrainerConfig::Remyr(cfg) => _train::<RemyrTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            output_path,
            &mut rng,
        ),
        TrainerConfig::DelayMultiplier(cfg) => _train::<DelayMultiplierTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            output_path,
            &mut rng,
        ),
    };

    Ok(())
}
