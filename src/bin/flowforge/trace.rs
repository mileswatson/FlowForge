use anyhow::Result;
use append_only_vec::AppendOnlyVec;
use flowforge::{
    components::ticker::Ticker,
    util::{meters::CurrentFlowMeter, never::Never, rand::Rng, WithLifetime},
    flow::{UtilityConfig, UtilityFunction},
    networks::{
        remy::{HasNetworkSubEffects, RemyNetworkConfig},
        NetworkBuilder, NetworkConfig,
    },
    quantities::{milliseconds, seconds, Float, InformationRate, Time, TimeSpan},
    simulation::{DynComponent, SimulatorBuilder},
    trainers::{delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer},
    CcaTemplate, Config, Trainer,
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
struct TraceResult<N> {
    active_senders: Vec<usize>,
    network: N,
    timestamps: Vec<Float>,
    aggregate_utility: Vec<Float>,
    flows: Vec<FlowTrace>,
}

fn _trace<T, N>(
    network_config: &impl NetworkConfig<NetworkBuilder = N>,
    utility_config: &UtilityConfig,
    input_path: &Path,
    rng: &mut Rng,
) -> TraceResult<N>
where
    N: NetworkBuilder,
    T: Trainer,
    for<'sim> <T::DefaultEffectGenerator as WithLifetime>::Type<'sim>:
        HasNetworkSubEffects<'sim, <T::DefaultEffectGenerator as WithLifetime>::Type<'sim>>,
{
    let dna = T::Dna::load(input_path).unwrap();
    let n = rng.sample(network_config);
    let mut active_senders = Vec::new();
    let mut timestamps = Vec::new();
    let mut aggregate_utility = Vec::new();
    let result_flows = RefCell::new(Vec::<FlowTrace>::new());
    make_guard!(guard);
    let flows = AppendOnlyVec::<RefCell<CurrentFlowMeter>>::new();
    let cca_template = T::CcaTemplate::default();
    let cca_gen = cca_template.with(&dna);
    let builder = SimulatorBuilder::new(guard);
    builder.insert(DynComponent::<Never, _>::new(Ticker::new(
        milliseconds(1.),
        |time| {
            timestamps.push((time - Time::SIM_START).seconds());
            let properties = flows
                .iter()
                .map(|x: &RefCell<_>| x.borrow().current_properties(time))
                .collect_vec();
            let active_properties = properties
                .iter()
                .filter_map(|x| x.clone().ok())
                .collect_vec();
            active_senders.push(active_properties.len());
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
                    result_flows.borrow_mut()[i].add(throughput, rtt, utility)
                });
            aggregate_utility.push(
                utility_config
                    .total_utility(&active_properties)
                    .map(|(u, _)| u)
                    .unwrap_or(Float::NAN),
            );
        },
    )));
    let new_flow = || {
        let index = flows.push(RefCell::new(CurrentFlowMeter::new_disabled(
            Time::SIM_START,
            seconds(0.1),
        )));
        result_flows.borrow_mut().push(FlowTrace::default());
        &flows[index]
    };
    let sim = n.populate_sim::<_, T::DefaultEffectGenerator, _>(builder, &cca_gen, rng, new_flow);
    sim.run_while(|t| t < Time::from_sim_start(seconds(100.)));
    TraceResult {
        active_senders,
        network: n,
        timestamps,
        aggregate_utility,
        flows: result_flows.into_inner(),
    }
}

pub fn trace(
    mode: &FlowAdders,
    network_config: &Path,
    utility_config: &Path,
    input_path: &Path,
    output_path: Option<&Path>,
    seed: u64,
) -> Result<()> {
    let network_config = RemyNetworkConfig::load(network_config)?;
    let utility_config = UtilityConfig::load(utility_config)?;

    let mut rng = Rng::from_seed(seed);

    let result = match mode {
        FlowAdders::Remy => {
            _trace::<RemyTrainer, _>(&network_config, &utility_config, input_path, &mut rng)
        }
        FlowAdders::DelayMultiplier => _trace::<DelayMultiplierTrainer, _>(
            &network_config,
            &utility_config,
            input_path,
            &mut rng,
        ),
        FlowAdders::Remyr => {
            _trace::<RemyrTrainer, _>(&network_config, &utility_config, input_path, &mut rng)
        }
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
