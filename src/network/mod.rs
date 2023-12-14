use crate::{
    logging::NothingLogger,
    rand::Rng,
    simulation::{
        ComponentId, ComponentSlot, DynComponent, HasVariant, Simulator, SimulatorBuilder,
    },
    time::{Float, Rate, TimeSpan},
};

use self::link::{Link, Routable};

pub mod config;
pub mod link;
pub mod protocols;
pub mod toggler;

#[derive(Debug)]
pub struct Network {
    pub rtt: TimeSpan,
    pub packet_rate: Rate,
    pub loss_rate: Float,
    pub buffer_size: Option<usize>,
    pub num_senders: usize,
}

pub struct NetworkSlots<'a, 'b, E> {
    pub sender_slots: Vec<ComponentSlot<'a, 'b, E>>,
    pub sender_link_id: ComponentId,
}

impl Network {
    #[must_use]
    pub fn to_sim<'a, E, P, R>(
        &self,
        rng: &'a mut Rng,
        populate_components: impl FnOnce(NetworkSlots<'a, '_, E>, &mut Rng) -> R,
    ) -> (Simulator<'a, E, NothingLogger>, R)
    where
        E: HasVariant<P> + 'a,
        P: Routable + 'a,
    {
        let builder = SimulatorBuilder::new();
        let slots = NetworkSlots {
            sender_slots: (0..self.num_senders)
                .map(|_| builder.reserve_slot())
                .collect(),
            sender_link_id: builder.insert(DynComponent::new(Link::create(
                0.5 * self.rtt,
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
