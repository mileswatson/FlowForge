use std::rc::Rc;

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    average::{AveragePair, IterAverage, SameEmptiness},
    flow::{Flow, FlowProperties, NoActiveFlows, UtilityFunction},
    network::{config::NetworkConfig, Network, NetworkSlots},
    rand::Rng,
    time::{Float, Time, TimeSpan},
};

pub trait PopulateComponents: Sync {
    /// Populates senders and receiver slots
    fn populate_components<'a>(
        &'a self,
        network_slots: NetworkSlots<'a, '_>,
        rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow + 'a>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationConfig {
    network_samples: u32,
    run_sim_for: Float,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            network_samples: 1000,
            run_sim_for: 120.,
        }
    }
}

impl EvaluationConfig {
    pub fn evaluate(
        &self,
        network_config: &NetworkConfig,
        components: &impl PopulateComponents,
        utility_function: &(impl UtilityFunction + ?Sized),
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        let run_sim_for = TimeSpan::new(self.run_sim_for);
        let score_network = |(n, mut rng): (Network, Rng)| {
            let (sim, flows) = n.to_sim(&mut rng, |slots, rng| {
                components.populate_components(slots, rng)
            });
            sim.run_for(run_sim_for);
            utility_function.total_utility(&flows, Time::sim_start() + run_sim_for)
        };

        (0..self.network_samples)
            .map(|_| (rng.sample(network_config), rng.create_child()))
            .par_bridge()
            .map(score_network)
            .filter_map(Result::ok)
            .map(AveragePair::new)
            .collect::<Vec<_>>()
            .average()
            .assert_same_emptiness()
            .map_err(|_| NoActiveFlows)
    }
}
