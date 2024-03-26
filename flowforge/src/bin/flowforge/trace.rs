use anyhow::Result;
use flowforge::{
    core::{
        meters::{CurrentFlowMeter, FlowNotActive},
        never::Never,
        rand::Rng,
    },
    flow::{FlowNeverActive, UtilityConfig, UtilityFunction},
    network::{
        config::NetworkConfig, ticker::Ticker, AddFlows, EffectTypeGenerator, HasNetworkSubEffects,
        Network,
    },
    protocols::remy::dna::RemyDna,
    quantities::{milliseconds, seconds, Float, InformationRate, Time, TimeSpan},
    simulation::DynComponent,
    trainers::{delay_multiplier::DelayMultiplierFlowAdder, remy::RemyFlowAdder, DefaultEffect},
    Config, Dna,
};
use generativity::make_guard;
use itertools::Itertools;
use serde::Serialize;
use std::{cell::RefCell, fs::File, path::Path};

use crate::FlowAdders;

#[derive(Serialize, Default, Clone)]
struct FlowTrace {
    bandwidth_kbps: Vec<Float>,
    rtt_ms: Vec<Float>,
    utility: Vec<Float>,
}

impl FlowTrace {
    pub fn add(&mut self, bandwidth: InformationRate, rtt: TimeSpan, utility: Float) {
        self.bandwidth_kbps
            .push(bandwidth.bits_per_second() / 1000.);
        self.rtt_ms.push(rtt.milliseconds());
        self.utility.push(utility);
    }
}

#[derive(Serialize)]
struct TraceResult {
    active_senders: Vec<usize>,
    network: Network,
    timestamps: Vec<Float>,
    aggregate_utility: Vec<Float>,
    flows: Vec<FlowTrace>,
}

impl TraceResult {
    pub fn new(network: Network) -> TraceResult {
        TraceResult {
            timestamps: Vec::new(),
            aggregate_utility: Vec::new(),
            flows: vec![FlowTrace::default(); network.num_senders as usize],
            network,
            active_senders: Vec::new(),
        }
    }
}

fn _trace<A, G>(
    network_config: &NetworkConfig,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) -> TraceResult
where
    A: AddFlows<G>,
    A::Dna: Dna + 'static,
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasNetworkSubEffects<'sim, G::Type<'sim>>,
{
    let dna = A::Dna::load(input_path).unwrap();
    let n = rng.sample(network_config);
    let mut result = TraceResult::new(n.clone());
    make_guard!(guard);
    let flows = (0..n.num_senders)
        .map(|_| {
            RefCell::new(CurrentFlowMeter::new_disabled(
                Time::SIM_START,
                seconds(0.1),
            ))
        })
        .collect_vec();
    let sim = n.to_sim::<A, _, _>(guard, rng, &flows, &dna, |builder| {
        builder.insert(DynComponent::<Never, _>::new(Ticker::new(
            milliseconds(1.),
            |time| {
                result.timestamps.push((time - Time::SIM_START).seconds());
                let properties = flows
                    .iter()
                    .map(|x| x.borrow().current_properties(time))
                    .collect_vec();
                let active_properties = properties
                    .iter()
                    .filter_map(|x| x.clone().ok())
                    .collect_vec();
                result.active_senders.push(active_properties.len());
                flows
                    .iter()
                    .zip(properties)
                    .map(|(f, p)| {
                        (
                            f.borrow().current_bandwidth(time),
                            f.borrow().current_rtt(time).unwrap_or(seconds(Float::NAN)),
                            p.map(|p| utility_config.flow_utility(&p))
                                .unwrap_or(Float::NAN),
                        )
                    })
                    .enumerate()
                    .for_each(|(i, (throughput, rtt, utility))| {
                        result.flows[i].add(throughput, rtt, utility)
                    });
                result.aggregate_utility.push(
                    utility_config
                        .total_utility(&active_properties)
                        .map(|(u, _)| u)
                        .unwrap_or(Float::NAN),
                );
            },
        )));
    });
    sim.run_for(seconds(100.));
    result
}

pub fn trace(
    mode: &FlowAdders,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
    output_path: Option<&Path>,
    seed: u64,
) -> Result<()> {
    let network_config = NetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(seed);

    let result = match mode {
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

    if let Some(output_path) = output_path {
        let file = File::create(output_path).unwrap();
        serde_json::to_writer(file, &result).unwrap();
        println!("{}", serde_json::to_string(&result.network).unwrap());
    } else {
        println!("{}", serde_json::to_string(&result).unwrap());
    }

    Ok(())
}