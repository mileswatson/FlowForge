use generativity::Guard;

use crate::{
    logging::NothingLogger,
    quantities::{packets, Float, Information, InformationRate, Time, TimeSpan},
    rand::{PositiveContinuousDistribution, Rng},
    simulation::{
        ComponentId, ComponentSlot, DynComponent, MaybeHasVariant, Message, Simulator,
        SimulatorBuilder,
    },
};

use self::{
    link::Link,
    toggler::{Toggle, Toggler},
};

pub mod config;
pub mod link;
pub mod protocols;
pub mod toggler;

#[derive(Debug)]
pub struct Packet<'sim> {
    seq: u64,
    source: ComponentId<'sim>,
    destination: ComponentId<'sim>,
    sent_time: Time,
}

impl<'sim> Packet<'sim> {
    fn pop_next_hop(&mut self) -> ComponentId<'sim> {
        self.destination
    }

    #[allow(clippy::unused_self)]
    const fn size(&self) -> Information {
        packets(1)
    }
}

#[derive(Debug)]
pub enum NetworkEffect<'sim> {
    Packet(Packet<'sim>),
    Toggle(Toggle),
}

pub type NetworkMessage<'sim> = Message<'sim, NetworkEffect<'sim>>;

impl<'sim> MaybeHasVariant<'sim, Toggle> for NetworkEffect<'sim> {
    fn try_into(self) -> Result<Toggle, Self> {
        match self {
            NetworkEffect::Packet(_) => Err(self),
            NetworkEffect::Toggle(t) => Ok(t),
        }
    }
}

impl<'sim> From<Toggle> for NetworkEffect<'sim> {
    fn from(value: Toggle) -> Self {
        NetworkEffect::Toggle(value)
    }
}

impl<'sim> MaybeHasVariant<'sim, Packet<'sim>> for NetworkEffect<'sim> {
    fn try_into(self) -> Result<Packet<'sim>, Self> {
        match self {
            NetworkEffect::Packet(p) => Ok(p),
            NetworkEffect::Toggle(_) => Err(self),
        }
    }
}

impl<'sim> From<Packet<'sim>> for NetworkEffect<'sim> {
    fn from(value: Packet<'sim>) -> Self {
        NetworkEffect::Packet(value)
    }
}

#[derive(Debug)]
pub struct Network {
    pub rtt: TimeSpan,
    pub packet_rate: InformationRate,
    pub loss_rate: Float,
    pub buffer_size: Option<Information>,
    pub num_senders: usize,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

pub struct NetworkSlots<'sim, 'a, 'b> {
    pub sender_slots: Vec<ComponentSlot<'sim, 'a, 'b, NetworkEffect<'sim>>>,
    pub sender_link_id: ComponentId<'sim>,
}

impl Network {
    #[must_use]
    pub fn to_sim<'sim, 'a, R>(
        &self,
        guard: Guard<'sim>,
        rng: &'a mut Rng,
        populate_components: impl FnOnce(NetworkSlots<'sim, 'a, '_>, &mut Rng) -> R + 'a,
    ) -> (Simulator<'sim, 'a, NetworkEffect<'sim>, NothingLogger>, R) {
        let builder = SimulatorBuilder::<'sim, '_>::new(guard);
        let slots = NetworkSlots {
            sender_slots: (0..self.num_senders)
                .map(|_| {
                    let slot = builder.reserve_slot();
                    builder.insert(DynComponent::new(Toggler::new(
                        slot.id(),
                        self.on_time.clone(),
                        self.off_time.clone(),
                        rng,
                    )));
                    slot
                })
                .collect(),
            sender_link_id: builder.insert(DynComponent::new(Link::create(
                self.rtt,
                self.packet_rate,
                self.loss_rate,
                self.buffer_size,
                NothingLogger,
            ))),
        };

        let r = populate_components(slots, rng);

        (builder.build(rng, NothingLogger), r)
    }
}
