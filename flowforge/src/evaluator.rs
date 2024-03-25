use generativity::make_guard;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        average::{AveragePair, IterAverage, SameEmptiness},
        meters::AverageFlowMeter,
        never::Never,
        rand::Rng,
    },
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    network::{
        config::NetworkConfig, toggler::Toggle, AddFlows, EffectTypeGenerator, Network, Packet,
    },
    quantities::{seconds, Float, Time, TimeSpan},
    simulation::HasSubEffect,
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
            network_samples: 1000,
            run_sim_for: seconds(120.),
        }
    }
}

impl EvaluationConfig {
    pub fn evaluate<A, G>(
        &self,
        network_config: &NetworkConfig,
        dna: &A::Dna,
        utility_function: &(impl UtilityFunction + ?Sized),
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>
    where
        A: AddFlows<G>,
        A::Dna: Sync,
        G: EffectTypeGenerator,
        for<'sim> G::Type<'sim>:
            HasSubEffect<Packet<'sim, G::Type<'sim>>> + HasSubEffect<Toggle> + HasSubEffect<Never>,
    {
        let score_network = |(n, mut rng): (Network, Rng)| {
            make_guard!(guard);
            let mut flows = (0..n.num_senders)
                .map(|_| AverageFlowMeter::new_disabled())
                .collect_vec();
            let sim = n.to_sim::<A, _, _>(guard, &mut rng, &mut flows, dna);
            sim.run_for(self.run_sim_for);
            let flow_stats = flows
                .iter()
                .map(|x| x.average_properties(Time::SIM_START + self.run_sim_for))
                .collect_vec();
            utility_function.total_utility(&flow_stats)
        };

        let networks = (0..self.network_samples)
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
