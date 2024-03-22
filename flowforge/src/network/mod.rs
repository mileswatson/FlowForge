use std::{fmt::Debug, rc::Rc};

use derive_where::derive_where;
use generativity::Guard;

use crate::{
    flow::Flow,
    logging::NothingLogger,
    never::Never,
    quantities::{packets, Float, Information, InformationRate, Time, TimeSpan},
    rand::{PositiveContinuousDistribution, Rng},
    simulation::{Address, DynComponent, HasSubEffect, Simulator, SimulatorBuilder},
};

use self::{
    link::Link,
    toggler::{Toggle, Toggler},
};

pub mod bouncer;
pub mod config;
pub mod link;
pub mod senders;
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

#[derive(Debug)]
pub struct Network {
    pub rtt: TimeSpan,
    pub packet_rate: InformationRate,
    pub loss_rate: Float,
    pub buffer_size: Option<Information>,
    pub num_senders: u32,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

pub struct PopulateComponentsResult<'sim, 'a, E> {
    pub senders: Vec<Address<'sim, Toggle, E>>,
    pub flows: Vec<Rc<dyn Flow + 'a>>,
}

pub trait EffectTypeGenerator {
    type Type<'a>;
}

pub trait PopulateComponents<G>: Sync
where
    G: EffectTypeGenerator,
{
    /// Populates senders and receiver slots
    fn populate_components<'sim>(
        &'sim self,
        num_senders: u32,
        simulator_builder: &mut SimulatorBuilder<'sim, 'sim, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        rng: &mut Rng,
    ) -> PopulateComponentsResult<'sim, 'sim, G::Type<'sim>>
    where
        G::Type<'sim>: 'sim;
}

impl Network {
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn to_sim<'sim, G>(
        &self,
        guard: Guard<'sim>,
        rng: &'sim mut Rng,
        populate_components: &'sim impl PopulateComponents<G>,
    ) -> (
        Simulator<'sim, 'sim, G::Type<'sim>, NothingLogger>,
        Vec<Rc<dyn Flow + 'sim>>,
    )
    where
        G: EffectTypeGenerator,
        G::Type<'sim>: HasSubEffect<Packet<'sim, G::Type<'sim>>>
            + HasSubEffect<Toggle>
            + HasSubEffect<Never>
            + 'sim,
    {
        let mut builder = SimulatorBuilder::<'sim, '_>::new(guard);
        let sender_link_id = builder.insert(DynComponent::new(Link::create(
            self.rtt,
            self.packet_rate,
            self.loss_rate,
            self.buffer_size,
            rng.create_child(),
            NothingLogger,
        )));
        let PopulateComponentsResult { senders, flows } = populate_components.populate_components(
            self.num_senders,
            &mut builder,
            sender_link_id,
            rng,
        );
        for sender in senders {
            builder.insert(DynComponent::new(Toggler::new(
                sender,
                self.on_time.clone(),
                self.off_time.clone(),
                rng.create_child(),
            )));
        }

        (builder.build(NothingLogger), flows)
    }
}
