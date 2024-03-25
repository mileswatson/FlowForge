use anyhow::Result;
use flowforge::{
    core::{meters::CurrentFlowMeter, rand::Rng},
    flow::UtilityConfig,
    network::{config::NetworkConfig, AddFlows, EffectTypeGenerator, HasNetworkSubEffects},
    protocols::remy::dna::RemyDna,
    quantities::{seconds, Float, Time, TimeSpan},
    trainers::{delay_multiplier::DelayMultiplierFlowAdder, remy::RemyFlowAdder, DefaultEffect},
    Config, Dna,
};
use generativity::make_guard;
use itertools::Itertools;
use serde::Serialize;
use std::path::Path;

use crate::FlowAdders;

#[derive(Serialize)]
struct FlowTrace {
    bandwidth_kbps: Vec<Float>,
    rtt_ms: Vec<Float>,
    utility: Vec<Float>,
}

#[derive(Serialize)]
struct TraceResult {
    timestamps: Vec<Float>,
    aggregate_utility: Vec<Float>,
    flows: Vec<FlowTrace>,
}

pub fn _trace<A, G>(
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) where
    A: AddFlows<G>,
    A::Dna: Dna,
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasNetworkSubEffects<'sim, G::Type<'sim>>,
{
    let dna = A::Dna::load(input_path).unwrap();
    let n = rng.sample(network_config);
    make_guard!(guard);
    let mut flows = (0..n.num_senders)
        .map(|_| CurrentFlowMeter::new_enabled(Time::SIM_START, seconds(0.5)))
        .collect_vec();
    let sim = n.to_sim::<A, _, _>(guard, rng, &mut flows, &dna);
    sim.run_for(seconds(100.));
}

pub fn trace(
    mode: &FlowAdders,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
    seed: u64,
) -> Result<()> {
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(seed);

    match mode {
        FlowAdders::Remy => _trace::<RemyFlowAdder<RemyDna>, DefaultEffect>(
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
        FlowAdders::DelayMultiplier => _trace::<DelayMultiplierFlowAdder, DefaultEffect>(
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
    };

    Ok(())
}
