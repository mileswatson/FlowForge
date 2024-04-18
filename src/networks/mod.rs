use rand_distr::Distribution;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    core::{logging::NothingLogger, meters::FlowMeter, rand::Rng, WithLifetime},
    simulation::{Simulator, SimulatorBuilder},
    Cca,
};

use self::remy::HasNetworkSubEffects;

pub mod remy;

pub trait NetworkBuilder: Clone + Send {
    fn populate_sim<'sim, 'a, C, G, F>(
        &self,
        builder: SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        new_cca: impl Fn() -> C + Clone + 'a,
        rng: &'a mut Rng,
        new_flow_meter: impl FnMut() -> F,
    ) -> Simulator<'sim, 'a, G::Type<'sim>, NothingLogger>
    where
        C: Cca + 'a,
        G: WithLifetime,
        G::Type<'sim>: HasNetworkSubEffects<'sim, G::Type<'sim>>,
        F: FlowMeter + 'a,
        'sim: 'a;
}

pub trait NetworkConfig:
    Serialize + DeserializeOwned + Distribution<Self::NetworkBuilder> + Sync
{
    type NetworkBuilder: NetworkBuilder;
}
