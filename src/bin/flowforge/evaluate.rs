use std::path::Path;

use anyhow::Result;
use flowforge::{
    eval::EvaluationConfig,
    flow::{FlowProperties, UtilityConfig},
    networks::DefaultNetworkConfig,
    quantities::Float,
    trainers::{
        delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer,
        DefaultEffect,
    },
    util::rand::Rng,
    CcaTemplate, Config, NetworkDistribution, Trainer,
};

use crate::FlowAdders;

pub fn _evaluate<T>(
    evaluation_config: &EvaluationConfig,
    network_config: &impl NetworkDistribution<DefaultEffect<'static>>,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) -> (Float, FlowProperties)
where
    T: Trainer,
{
    let dna = T::Dna::load(input_path).unwrap();

    let x = evaluation_config
        .evaluate(
            T::CcaTemplate::default().with(&dna),
            network_config,
            utility_config,
            &mut rng.identical_child_factory()(),
        )
        .expect("Expected active flows!");
    #[allow(clippy::let_and_return)]
    x
}

pub fn evaluate(
    mode: &FlowAdders,
    evaluation_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
    eval_seed: u64,
) -> Result<()> {
    let mut rng = Rng::from_seed(eval_seed);
    let evaluation_config = EvaluationConfig::load(evaluation_config)?;
    let network_config = DefaultNetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let (score, flow_properties) = match mode {
        FlowAdders::Remy => _evaluate::<RemyTrainer>(
            &evaluation_config,
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
        FlowAdders::DelayMultiplier => _evaluate::<DelayMultiplierTrainer>(
            &evaluation_config,
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
        FlowAdders::Remyr => _evaluate::<RemyrTrainer>(
            &evaluation_config,
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
