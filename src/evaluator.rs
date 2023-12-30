use std::rc::Rc;

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    flow::{Flow, UtilityFunction},
    network::{config::NetworkConfig, link::Routable, toggler::Toggle, Network, NetworkSlots},
    rand::Rng,
    simulation::HasVariant,
    time::{Float, Time, TimeSpan},
};

pub trait PopulateComponents<E>: Sync {
    /// Populates senders and receiver slots
    fn populate_components<'a>(
        &'a self,
        network_slots: NetworkSlots<'a, '_, E>,
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
    pub fn evaluate<E, P>(
        &self,
        network_config: &NetworkConfig,
        components: &impl PopulateComponents<E>,
        utility_function: &(impl UtilityFunction + ?Sized),
        rng: &mut Rng,
    ) -> Float
    where
        E: HasVariant<P> + HasVariant<Toggle>,
        P: Routable,
    {
        let run_sim_for = TimeSpan::new(self.run_sim_for);
        let score_network = |(n, mut rng): (Network, Rng)| -> Float {
            let (sim, flows) = n.to_sim::<E, P, _>(&mut rng, |slots, rng| {
                let x = components.populate_components(slots, rng);
                x
            });
            sim.run_for(run_sim_for);
            utility_function
                .total_utility(&flows, Time::sim_start() + run_sim_for)
                .unwrap_or(Float::MIN)
        };
        #[allow(clippy::cast_precision_loss)]
        return (0..self.network_samples)
            .map(|_| (rng.sample(network_config), rng.create_child()))
            .par_bridge()
            .map(score_network)
            .sum::<Float>()
            / f64::from(self.network_samples);
    }
}
