use std::{
    fs::File,
    io::{self, Seek, Write},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use flowforge::{
    eval::EvaluationConfig,
    flow::{FlowProperties, UtilityConfig},
    networks::DefaultNetworkConfig,
    quantities::Float,
    trainers::{
        delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer,
        DefaultEffect, TrainerConfig,
    },
    util::rand::Rng,
    CcaTemplate, Config, NetworkDistribution, Trainer,
};
use serde::Serialize;

#[derive(Serialize)]
struct TrainResult<'a, T, N> {
    timestamps: Vec<Float>,
    bandwidth: Vec<Float>,
    rtt: Vec<Float>,
    utility: Vec<Float>,
    trainer_config: &'a T,
    network_config: &'a N,
    utility_config: &'a UtilityConfig,
}

impl<'a, T, N> TrainResult<'a, T, N> {
    pub fn new(
        trainer_config: &'a T,
        network_config: &'a N,
        utility_config: &'a UtilityConfig,
    ) -> TrainResult<'a, T, N> {
        TrainResult {
            timestamps: Vec::new(),
            bandwidth: Vec::new(),
            rtt: Vec::new(),
            utility: Vec::new(),
            trainer_config,
            network_config,
            utility_config,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn _train<T>(
    trainer: &T,
    evaluation_config: Option<(u32, EvaluationConfig, Option<&Path>)>,
    network_config: &(impl NetworkDistribution<DefaultEffect<'static>> + Serialize),
    utility_config: &UtilityConfig,
    dna_path: &Path,
    training_rng: &mut Rng,
    eval_rng: &mut Rng,
    force: bool,
) where
    T: Trainer + Serialize + Sync,
{
    assert!(T::Dna::valid_path(dna_path));
    if dna_path.exists() && !force {
        loop {
            let mut buf = String::new();
            println!("There is already DNA in the output path. Are you sure you want to overwrite it? y/n");
            std::io::stdin().read_line(&mut buf).unwrap();
            if buf.to_lowercase().trim() == "y" {
                break;
            } else if buf.to_lowercase().trim() == "n" {
                return;
            }
        }
    }
    let mut output_file = evaluation_config
        .as_ref()
        .and_then(|x| x.2)
        .map(|x| File::create(x).unwrap());
    let mut result = TrainResult::new(trainer, network_config, utility_config);

    let mut last_resumed = Instant::now();
    let mut total_training_time = Duration::ZERO;

    let new_eval_rng = eval_rng.identical_child_factory();
    let mut last_percent = -1;
    let mut best_score: Float = Float::MIN;
    trainer
        .train(
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

                    let now = Instant::now();
                    total_training_time += now - last_resumed;

                    print!("Evaluating... ");
                    io::stdout().flush().unwrap();
                    let (utility, props) = evaluation_config
                        .evaluate::<_, DefaultEffect, _>(
                            &T::CcaTemplate::default().with(dna),
                            network_config,
                            utility_config,
                            &mut new_eval_rng(),
                        )
                        .expect("Simulation to have active flows");
                    let FlowProperties {
                        throughput: average_throughput,
                        rtt: average_rtt,
                    } = props.clone();
                    if let Some(output_file) = &mut output_file {
                        result.timestamps.push(total_training_time.as_secs_f64());
                        result.bandwidth.push(average_throughput.bits_per_second());
                        result.rtt.push(average_rtt.unwrap().seconds());
                        result.utility.push(utility);
                        output_file.rewind().unwrap();
                        serde_json::to_writer(output_file, &result).unwrap();
                    }
                    dna.save(dna_path).unwrap();
                    if utility >= best_score {
                        best_score = utility;
                        println!("Achieved eval score {utility:.2} with {props}. Best so far.");
                    } else {
                        println!("Achieved eval score {utility:.2} with {props}.");
                    }

                    last_resumed = Instant::now();
                }
            },
            training_rng,
        )
        .save(dna_path)
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
pub fn train(
    trainer_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    dna_path: &Path,
    eval_times: Option<u32>,
    evaluation_config: Option<&Path>,
    output_path: Option<&Path>,
    force: bool,
    training_seed: u64,
    eval_seed: u64,
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
    let network_config = DefaultNetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut training_rng = Rng::from_seed(training_seed);
    let mut eval_rng = Rng::from_seed(eval_seed);

    match trainer_config {
        TrainerConfig::Remy(cfg) => _train::<RemyTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut training_rng,
            &mut eval_rng,
            force,
        ),
        TrainerConfig::Remyr(cfg) => _train::<RemyrTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut training_rng,
            &mut eval_rng,
            force,
        ),
        TrainerConfig::DelayMultiplier(cfg) => _train::<DelayMultiplierTrainer>(
            &cfg,
            evaluation_config,
            &network_config,
            &utility_config,
            dna_path,
            &mut training_rng,
            &mut eval_rng,
            force,
        ),
    };

    Ok(())
}
