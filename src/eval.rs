use std::cell::RefCell;

use append_only_vec::AppendOnlyVec;
use generativity::make_guard;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    quantities::{seconds, Float, Time, TimeSpan},
    simulation::SimulatorBuilder,
    util::{
        average::{AveragePair, IterAverage, NoItems, SameEmptiness},
        logging::NothingLogger,
        meters::AverageFlowMeter,
        rand::Rng,
        WithLifetime,
    },
    Cca, Network, NetworkDistribution,
};

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationConfig {
    pub network_samples: u32,
    pub run_sim_for: TimeSpan,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            network_samples: 500,
            run_sim_for: seconds(60.),
        }
    }
}

impl EvaluationConfig {
    pub fn evaluate<C, G, B>(
        &self,
        new_cca: impl Fn() -> C + Sync,
        network_config: &impl NetworkDistribution<G, Network = B>,
        utility_function: &(impl UtilityFunction + ?Sized),
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>
    where
        B: Network<G>,
        C: Cca,
        G: WithLifetime,
    {
        let score_network = |(n, mut rng): (B, Rng)| {
            let flows = AppendOnlyVec::new();
            let new_flow = || {
                let index = flows.push(RefCell::new(AverageFlowMeter::new_disabled()));
                &flows[index]
            };
            make_guard!(guard);
            let builder = SimulatorBuilder::new(guard);
            n.populate_sim(&builder, &new_cca, &mut rng, new_flow);
            let sim = builder.build(NothingLogger).unwrap();
            let sim_end = Time::from_sim_start(self.run_sim_for);
            sim.run_while(|t| t < sim_end);
            let flow_stats = flows
                .iter()
                .filter_map(|x| x.borrow().average_properties(sim_end).ok())
                .collect_vec();

            (
                utility_function.utility(&flow_stats).map_err(|_| NoItems),
                flow_stats.average(),
            )
                .assert_same_emptiness()
        };

        let networks: Vec<_> = (0..self.network_samples)
            .map(|_| (rng.sample(network_config), rng.create_child()))
            .collect_vec();
        networks
            .into_par_iter()
            .map(score_network)
            .filter_map(Result::ok)
            .map(AveragePair::new)
            .collect::<Vec<_>>()
            .average()
            .assert_same_emptiness()
            .map_err(|_| NoActiveFlows)
    }
}
