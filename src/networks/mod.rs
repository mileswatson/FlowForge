use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    simulation::SimulatorBuilder,
    util::{meters::FlowMeter, rand::Rng, WithLifetime},
    Cca, Network, NetworkDistribution,
};

use self::remy::{HasRemyNetworkSubEffects, RemyNetworkBuilder, RemyNetworkConfig};

pub mod remy;

pub trait HasDefaultNetworkSubEffects<'sim, E>: HasRemyNetworkSubEffects<'sim, E> {}

impl<'sim, E, T> HasDefaultNetworkSubEffects<'sim, E> for T where
    T: HasRemyNetworkSubEffects<'sim, E>
{
}

#[derive(Serialize, Deserialize)]
pub enum DefaultNetworkConfig {
    Remy(RemyNetworkConfig),
}

impl Default for DefaultNetworkConfig {
    fn default() -> Self {
        DefaultNetworkConfig::Remy(RemyNetworkConfig::default())
    }
}

#[derive(Clone, Serialize)]
pub enum DefaultNetworkBuilder {
    Remy(RemyNetworkBuilder),
}

impl Distribution<DefaultNetworkBuilder> for DefaultNetworkConfig {
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> DefaultNetworkBuilder {
        match self {
            DefaultNetworkConfig::Remy(cfg) => DefaultNetworkBuilder::Remy(rng.sample(cfg)),
        }
    }
}

impl<G> NetworkDistribution<G> for DefaultNetworkConfig
where
    G: WithLifetime,
    for<'sim> G::Type<'sim>: HasDefaultNetworkSubEffects<'sim, G::Type<'sim>>,
{
    type Network = DefaultNetworkBuilder;
}

impl<G> Network<G> for DefaultNetworkBuilder
where
    G: WithLifetime,
    for<'sim> G::Type<'sim>: HasDefaultNetworkSubEffects<'sim, G::Type<'sim>>,
{
    fn populate_sim<'sim, 'a, C, F>(
        &self,
        builder: &SimulatorBuilder<'sim, 'a, <G as WithLifetime>::Type<'sim>>,
        new_cca: impl Fn() -> C + Clone + 'a,
        rng: &'a mut Rng,
        new_flow_meter: impl FnMut() -> F,
    ) where
        C: Cca + 'a,

        F: FlowMeter + 'a,
        'sim: 'a,
    {
        match self {
            DefaultNetworkBuilder::Remy(n) => {
                <RemyNetworkBuilder as Network<G>>::populate_sim(
                    n,
                    builder,
                    new_cca,
                    rng,
                    new_flow_meter,
                );
            }
        }
    }
}
