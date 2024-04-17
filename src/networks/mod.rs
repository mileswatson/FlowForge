use generativity::Guard;
use serde::Serialize;

use crate::{
    components::{
        link::Link, packet::Packet, senders::window::{
            LossyInternalControllerEffect, LossyInternalSenderEffect, LossyWindowSender,
        }, toggler::{Toggle, Toggler}
    },
    core::{
        logging::NothingLogger,
        meters::FlowMeter,
        never::Never,
        rand::{PositiveContinuousDistribution, Rng},
        WithLifetime,
    },
    quantities::{Float, Information, InformationRate, TimeSpan},
    simulation::{DynComponent, HasSubEffect, Simulator, SimulatorBuilder},
    Cca,
};

pub mod config;

#[derive(Debug, Clone, Serialize)]
pub struct RemyNetwork {
    pub rtt: TimeSpan,
    pub packet_rate: InformationRate,
    pub loss_rate: Float,
    pub buffer_size: Option<Information>,
    pub num_senders: u32,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

pub trait HasNetworkSubEffects<'sim, E>:
    HasSubEffect<LossyInternalSenderEffect<'sim, E>>
    + HasSubEffect<LossyInternalControllerEffect>
    + HasSubEffect<Packet<'sim, E>>
    + HasSubEffect<Toggle>
    + HasSubEffect<Never>
    + 'sim
{
}

impl<'sim, E, T> HasNetworkSubEffects<'sim, E> for T where
    T: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
        + HasSubEffect<LossyInternalControllerEffect>
        + HasSubEffect<Packet<'sim, E>>
        + HasSubEffect<Toggle>
        + HasSubEffect<Never>
        + 'sim
{
}

impl RemyNetwork {
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn to_sim<'sim, 'a, C, G, F>(
        &self,
        new_cca: impl Fn() -> C + Clone + 'a,
        guard: Guard<'sim>,
        rng: &'a mut Rng,
        mut new_flow_meter: impl FnMut() -> F,
        extra_components: impl FnOnce(&SimulatorBuilder<'sim, 'a, G::Type<'sim>>),
    ) -> Simulator<'sim, 'a, G::Type<'sim>, NothingLogger>
    where
        C: Cca + 'a,
        G: WithLifetime,
        G::Type<'sim>: HasNetworkSubEffects<'sim, G::Type<'sim>>,
        F: FlowMeter + 'a,
        'sim: 'a,
    {
        let builder = SimulatorBuilder::<'sim, '_>::new(guard);
        extra_components(&builder);
        let sender_link_id = builder.insert(DynComponent::new(Link::create(
            self.rtt,
            self.packet_rate,
            self.loss_rate,
            self.buffer_size,
            rng.create_child(),
            NothingLogger,
        )));
        for _ in 0..self.num_senders {
            let slot = LossyWindowSender::reserve_slot::<_>(&builder);
            let address = slot.address();
            let packet_address = address.clone().cast();
            slot.set(
                packet_address.clone(),
                sender_link_id.clone(),
                packet_address,
                new_cca.clone(),
                true,
                new_flow_meter(),
                rng.create_child(),
                NothingLogger,
            );
            builder.insert(DynComponent::new(Toggler::new(
                address.cast(),
                self.on_time.clone(),
                self.off_time.clone(),
                rng.create_child(),
            )));
        }

        builder.build(NothingLogger)
    }
}
