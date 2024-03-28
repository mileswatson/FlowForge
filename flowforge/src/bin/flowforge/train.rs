use std::{fs::File, io::Seek, path::Path, time::Instant};

use anyhow::Result;
use flowforge::{
    core::rand::{Rng, Wrapper},
    flow::UtilityConfig,
    network::config::NetworkConfig,
    quantities::{Float, InformationRate, TimeSpan},
    trainers::{
        delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer,
        TrainerConfig,
    },
    Config, Trainer,
};
use serde::Serialize;

pub fn _train<T>(
    trainer_config: &T::Config,
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    dna: &Path,
    output_path: Option<&Path>,
    rng: &mut Rng,
) where
    T: Trainer,
{
    assert!(T::Dna::valid_path(dna));
    let starting_point = T::Dna::load(dna).ok().and_then(|d| loop {
        let mut buf = String::new();
        println!("There is already valid DNA in the output path. Would you like to use it as a starting point? Y/N");
        std::io::stdin().read_line(&mut buf).unwrap();
        if buf.to_lowercase().trim() == "y" {
            return Some(d)
        } else if buf.to_lowercase().trim() == "n" { 
            return None
        }
    });
    let mut output_file = output_path.map(|x| File::create(x).unwrap());
    let mut result = TrainResult::default();
    let start = Instant::now();
    T::new(trainer_config)
        .train(
            starting_point,
            network_config,
            utility_config,
            &mut |d: Option<&T::Dna>, utility, bandwidth: InformationRate, rtt: TimeSpan| {
                if let Some(x) = d {
                    x.save(dna).unwrap();
                }
                if let Some(output_file) = &mut output_file {
                    result
                        .timestamps
                        .push((Instant::now() - start).as_secs_f64());
                    result.bandwidth.push(bandwidth.to_underlying());
                    result.rtt.push(rtt.to_underlying());
                    result.utility.push(utility);
                    output_file.rewind().unwrap();
                    serde_json::to_writer(output_file, &result).unwrap();
                }
            },
            rng,
        )
        .save(dna)
        .unwrap();
}

#[derive(Default, Serialize)]
struct TrainResult {
    timestamps: Vec<Float>,
    bandwidth: Vec<Float>,
    rtt: Vec<Float>,
    utility: Vec<Float>,
}

pub fn train(
    trainer_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    dna: &Path,
    output_path: Option<&Path>,
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
            dna,
            output_path,
            &mut rng,
        ),
        TrainerConfig::Remyr(cfg) => _train::<RemyrTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            dna,
            output_path,
            &mut rng,
        ),
        TrainerConfig::DelayMultiplier(cfg) => _train::<DelayMultiplierTrainer>(
            &cfg,
            &network_config,
            &utility_config,
            dna,
            output_path,
            &mut rng,
        ),
    };

    Ok(())
}
