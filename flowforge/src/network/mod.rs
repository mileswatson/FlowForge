use std::fmt::Debug;

use derive_where::derive_where;
use generativity::Guard;
use itertools::Itertools;

use crate::{
    core::{
        logging::NothingLogger,
        meters::FlowMeter,
        never::Never,
        rand::{PositiveContinuousDistribution, Rng},
    },
    quantities::{packets, Float, Information, InformationRate, Time, TimeSpan},
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

pub trait EffectTypeGenerator {
    type Type<'a>;
}

pub trait PopulateComponents<G>: Sync
where
    G: EffectTypeGenerator,
{
    /// Populates senders and receiver slots
    fn populate_components<'sim, 'a, F>(
        &'sim self,
        senders: impl IntoIterator<Item = F>,
        simulator_builder: &mut SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        rng: &mut Rng,
    ) -> Vec<Address<'sim, Toggle, G::Type<'sim>>>
    where
        F: FlowMeter + 'a,
        G::Type<'sim>: 'sim,
        'sim: 'a;
}

impl Network {
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn to_sim<'sim, 'a, F, G>(
        &self,
        guard: Guard<'sim>,
        rng: &'a mut Rng,
        flows: impl IntoIterator<Item = F>,
        populate_components: &'sim impl PopulateComponents<G>,
    ) -> Simulator<'sim, 'a, G::Type<'sim>, NothingLogger>
    where
        F: FlowMeter + 'a,
        G: EffectTypeGenerator,
        G::Type<'sim>: HasSubEffect<Packet<'sim, G::Type<'sim>>>
            + HasSubEffect<Toggle>
            + HasSubEffect<Never>
            + 'sim,
        'sim: 'a,
    {
        let flows = flows.into_iter().collect_vec();
        assert_eq!(flows.len(), self.num_senders as usize);
        let mut builder = SimulatorBuilder::<'sim, '_>::new(guard);
        let sender_link_id = builder.insert(DynComponent::new(Link::create(
            self.rtt,
            self.packet_rate,
            self.loss_rate,
            self.buffer_size,
            rng.create_child(),
            NothingLogger,
        )));
        let senders =
            populate_components.populate_components(flows, &mut builder, sender_link_id, rng);
        for sender in senders {
            builder.insert(DynComponent::new(Toggler::new(
                sender,
                self.on_time.clone(),
                self.off_time.clone(),
                rng.create_child(),
            )));
        }

        builder.build(NothingLogger)
    }
}
