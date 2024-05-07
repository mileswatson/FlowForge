use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    simulation::SimulatorBuilder,
    util::{meters::FlowMeter, rand::Rng, OfLifetime},
    Cca, Network, NetworkDistribution,
};

use self::remy::{HasRemyNetworkVariants, RemyNetwork, RemyNetworkDistribution};

pub mod remy;

pub trait HasDefaultNetworkVariants<'sim, E>: HasRemyNetworkVariants<'sim, E> {}

impl<'sim, E, T> HasDefaultNetworkVariants<'sim, E> for T where T: HasRemyNetworkVariants<'sim, E> {}

#[derive(Serialize, Deserialize)]
pub enum DefaultNetworkConfig {
    Remy(RemyNetworkDistribution),
}

impl Default for DefaultNetworkConfig {
    fn default() -> Self {
        DefaultNetworkConfig::Remy(RemyNetworkDistribution::default())
    }
}

#[derive(Clone, Serialize)]
pub enum DefaultNetworkBuilder {
    Remy(RemyNetwork),
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
    G: OfLifetime,
    for<'sim> G::Of<'sim>: HasDefaultNetworkVariants<'sim, G::Of<'sim>>,
{
    type Network = DefaultNetworkBuilder;
}

impl<G> Network<G> for DefaultNetworkBuilder
where
    G: OfLifetime,
    for<'sim> G::Of<'sim>: HasDefaultNetworkVariants<'sim, G::Of<'sim>>,
{
    fn populate_sim<'sim, 'a, C, F>(
        &self,
        builder: &SimulatorBuilder<'sim, 'a, <G as OfLifetime>::Of<'sim>>,
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
                <RemyNetwork as Network<G>>::populate_sim(n, builder, new_cca, rng, new_flow_meter);
            }
        }
    }
}
