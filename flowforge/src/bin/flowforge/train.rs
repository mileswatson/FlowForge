use std::{
    fs::File,
    io::{self, Seek, Write},
    path::Path,
    time::Instant,
};

use anyhow::Result;
use flowforge::{
    core::{
        never::Never,
        rand::{Rng, Wrapper},
    },
    evaluator::EvaluationConfig,
    flow::{FlowProperties, UtilityConfig},
    network::{config::NetworkConfig, toggler::Toggle, EffectTypeGenerator, Packet},
    quantities::Float,
    simulation::HasSubEffect,
    trainers::{
        delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer,
        TrainerConfig,
    },
    Config, Trainer,
};
use serde::Serialize;

pub fn _train<T>(
    trainer_config: &T::Config,
    evaluation_config: Option<(u32, EvaluationConfig, Option<&Path>)>,
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    dna_path: &Path,
    rng: &mut Rng,
) where
    T: Trainer,
    for<'sim> <T::DefaultEffectGenerator as EffectTypeGenerator>::Type<'sim>: HasSubEffect<Packet<'sim, <T::DefaultEffectGenerator as EffectTypeGenerator>::Type<'sim>>>
        + HasSubEffect<Toggle>
        + HasSubEffect<Never>,
{
    assert!(T::Dna::valid_path(dna_path));
    let starting_point = T::Dna::load(dna_path).ok().and_then(|d| loop {
        let mut buf = String::new();
        println!("There is already valid DNA in the output path. Would you like to use it as a starting point? Y/N");
        std::io::stdin().read_line(&mut buf).unwrap();
        if buf.to_lowercase().trim() == "y" {
            return Some(d)
        } else if buf.to_lowercase().trim() == "n" { 
            return None
        }
    });
    let mut output_file = evaluation_config
        .as_ref()
        .and_then(|x| x.2)
        .map(|x| File::create(x).unwrap());
    let mut result = TrainResult::default();
    let start = Instant::now();

    let new_eval_rng = rng.identical_child_factory();
    let mut last_percent = -1;
    let mut best_score: Float = Float::MIN;
    T::new(trainer_config).train(
        starting_point,
        network_config,
        utility_config,
        &mut |frac: Float, dna: &T::Dna| {
            println!("{frac}");
            if let Some((eval_times, evaluation_config, _)) = evaluation_config.as_ref() {
                let percent_completed = (frac * *eval_times as f64).floor() as i32;
                if percent_completed <= last_percent {
                    return;
                }
                last_percent = percent_completed;
                print!("Evaluating... ");
                io::stdout().flush().unwrap();
                let (utility, props) = evaluation_config
                    .evaluate(
                        &T::DefaultFlowAdder::default(),
                        network_config,
                        dna,
                        utility_config,
                        &mut new_eval_rng(),
                    )
                    .expect("Simulation to have active flows");
                let FlowProperties {
                    average_throughput,
                    average_rtt,
                } = props.clone();
                if let Some(output_file) = &mut output_file {
                    result
                        .timestamps
                        .push((Instant::now() - start).as_secs_f64());
                    result.bandwidth.push(average_throughput.to_underlying());
                    result.rtt.push(average_rtt.unwrap().to_underlying());
                    result.utility.push(utility);
                    output_file.rewind().unwrap();
                    serde_json::to_writer(output_file, &result).unwrap();
                }
                if utility >= best_score {
                    dna.save(dna_path).unwrap();
                    best_score = utility;
                    println!("Achieved eval score {utility:.2} with {props}. Best so far, saved.");
                } else {
                    println!("Achieved eval score {utility:.2} with {props}.");
                }
            }
        },
        rng,
    );
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
    dna_path: &Path,
    eval_times: Option<u32>,
    evaluation_config: Option<&Path>,
    output_path: Option<&Path>,
) -> Result<()> {
    if output_path.is_some() {
        assert!(evaluation_config.is_some());
    }
    if evaluation_config.is_some() {
        assert!(eval_times.is_some());
    }

    let evaluation_config = match evaluation_config {
        Some(c) => Some((eval_times.unwrap(), EvaluationConfig::load(c)?, output_path)),
        None => None,
    };

    let trainer_config = TrainerConfig::load(trainer_config)?;
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(534522);

    match trainer_config {
        TrainerConfig::Remy(cfg) => _train::<RemyTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut rng,
        ),
        TrainerConfig::Remyr(cfg) => _train::<RemyrTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut rng,
        ),
        TrainerConfig::DelayMultiplier(cfg) => _train::<DelayMultiplierTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut rng,
        ),
    };

    Ok(())
}
