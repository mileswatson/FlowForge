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
pub struct Packet {
    seq: u64,
    source: ComponentId,
    destination: ComponentId,
    sent_time: Time,
}

impl Packet {
    fn pop_next_hop(&mut self) -> ComponentId {
        self.destination
    }

    #[allow(clippy::unused_self)]
    const fn size(&self) -> Information {
        packets(1)
    }
}

#[derive(Debug)]
pub enum NetworkEffect {
    Packet(Packet),
    Toggle(Toggle),
}

pub type NetworkMessage = Message<NetworkEffect>;

impl MaybeHasVariant<Toggle> for NetworkEffect {
    fn try_into(self) -> Result<Toggle, Self> {
        match self {
            NetworkEffect::Packet(_) => Err(self),
            NetworkEffect::Toggle(t) => Ok(t),
        }
    }
}

impl From<Toggle> for NetworkEffect {
    fn from(value: Toggle) -> Self {
        NetworkEffect::Toggle(value)
    }
}

impl MaybeHasVariant<Packet> for NetworkEffect {
    fn try_into(self) -> Result<Packet, Self> {
        match self {
            NetworkEffect::Packet(p) => Ok(p),
            NetworkEffect::Toggle(_) => Err(self),
        }
    }
}

impl From<Packet> for NetworkEffect {
    fn from(value: Packet) -> Self {
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
    pub off_time: PositiveContinuousDistribution<Float>,
    pub on_time: PositiveContinuousDistribution<Float>,
}

pub struct NetworkSlots<'a, 'b> {
    pub sender_slots: Vec<ComponentSlot<'a, 'b, NetworkEffect>>,
    pub sender_link_id: ComponentId,
}

impl Network {
    #[must_use]
    pub fn to_sim<'a, R>(
        &self,
        rng: &'a mut Rng,
        populate_components: impl FnOnce(NetworkSlots<'a, '_>, &mut Rng) -> R + 'a,
    ) -> (Simulator<'a, NetworkEffect, NothingLogger>, R) {
        let builder = SimulatorBuilder::new();
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
