use std::fmt::Debug;

use derive_where::derive_where;
use generativity::Guard;
use serde::Serialize;

use crate::{
    core::{
        logging::NothingLogger,
        meters::FlowMeter,
        never::Never,
        rand::{PositiveContinuousDistribution, Rng},
    },
    network::senders::window::LossyWindowSender,
    quantities::{packets, Float, Information, InformationRate, Time, TimeSpan},
    simulation::{Address, DynComponent, HasSubEffect, Simulator, SimulatorBuilder},
    Cca,
};

use self::{
    link::Link,
    senders::window::{LossyInternalControllerEffect, LossyInternalSenderEffect},
    toggler::{Toggle, Toggler},
};

pub mod bouncer;
pub mod config;
pub mod link;
pub mod senders;
pub mod ticker;
pub mod toggler;

#[derive_where(Debug)]
pub struct Packet<'sim, E> {
    seq: u64,
    source: Address<'sim, Packet<'sim, E>, E>,
    destination: Address<'sim, Packet<'sim, E>, E>,
    sent_time: Time,
}

impl<'sim, E> Packet<'sim, E> {
    fn pop_next_hop(&mut self) -> Address<'sim, Packet<'sim, E>, E> {
        self.destination.clone()
    }

    #[allow(clippy::unused_self)]
    const fn size(&self) -> Information {
        packets(1)
    }
}

pub type PacketAddress<'sim, E> = Address<'sim, Packet<'sim, E>, E>;

#[derive(Debug, Clone, Serialize)]
pub struct Network {
    pub rtt: TimeSpan,
    pub packet_rate: InformationRate,
    pub loss_rate: Float,
    pub buffer_size: Option<Information>,
    pub num_senders: u32,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

pub trait EffectTypeGenerator {
    type Type<'a>;
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

impl Network {
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
        G: EffectTypeGenerator,
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
