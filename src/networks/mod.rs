use rand_distr::Distribution;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    simulation::{Simulator, SimulatorBuilder},
    util::{logging::NothingLogger, meters::FlowMeter, rand::Rng, WithLifetime},
    Cca,
};

use self::remy::HasRemyNetworkSubEffects;

pub mod remy;

pub trait NetworkBuilder<G>: Clone + Send
where
    G: WithLifetime,
{
    fn populate_sim<'sim, 'a, C, F>(
        &self,
        builder: SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        new_cca: impl Fn() -> C + Clone + 'a,
        rng: &'a mut Rng,
        new_flow_meter: impl FnMut() -> F,
    ) -> Simulator<'sim, 'a, G::Type<'sim>, NothingLogger>
    where
        C: Cca + 'a,

        F: FlowMeter + 'a,
        'sim: 'a;
}

pub trait NetworkConfig<G>:
    Serialize + DeserializeOwned + Distribution<Self::NetworkBuilder> + Sync
where
    G: WithLifetime,
{
    type NetworkBuilder: NetworkBuilder<G>;
}

pub trait HasDefaultNetworkSubEffects<'sim, E>: HasRemyNetworkSubEffects<'sim, E> {}

impl<'sim, E, T> HasDefaultNetworkSubEffects<'sim, E> for T where
    T: HasRemyNetworkSubEffects<'sim, E>
{
}
