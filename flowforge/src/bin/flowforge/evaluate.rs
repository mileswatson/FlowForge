use std::path::Path;

use anyhow::Result;
use flowforge::{
    core::{never::Never, rand::Rng},
    evaluator::EvaluationConfig,
    flow::{FlowProperties, UtilityConfig},
    network::{config::NetworkConfig, toggler::Toggle, EffectTypeGenerator, Packet},
    quantities::Float,
    simulation::HasSubEffect,
    trainers::{delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer},
    Config, Trainer,
};

use crate::FlowAdders;

pub fn _evaluate<T>(
    evaluation_config: &EvaluationConfig,
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) -> (Float, FlowProperties)
where
    T: Trainer,
    for<'sim> <T::DefaultEffectGenerator as EffectTypeGenerator>::Type<'sim>: HasSubEffect<Packet<'sim, <T::DefaultEffectGenerator as EffectTypeGenerator>::Type<'sim>>>
        + HasSubEffect<Toggle>
        + HasSubEffect<Never>,
{
    let dna = T::Dna::load(input_path).unwrap();

    evaluation_config
        .evaluate::<T::DefaultFlowAdder, T::DefaultEffectGenerator>(
            &T::DefaultFlowAdder::default(),
            network_config,
            &dna,
            utility_config,
            &mut rng.identical_child_factory()(),
        )
        .expect("Expected active flows!")
}

pub fn evaluate(
    mode: &FlowAdders,
    evaluation_config: &Path,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
) -> Result<()> {
    let evaluation_config = EvaluationConfig::load(evaluation_config)?;
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(534522);

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
