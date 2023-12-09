use crate::{
    logging::NothingLogger,
    rand::Rng,
    simulation::{ComponentId, DynComponent, HasVariant, Simulator},
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

pub struct SimProperties {
    pub sender_ids: Vec<ComponentId>,
    pub sender_link_id: ComponentId,
    pub receiver_id: ComponentId,
    pub receiver_link_id: ComponentId,
}

impl Network {
    pub fn sim_properties(&self) -> SimProperties {
        SimProperties {
            sender_ids: (0..self.num_senders).map(ComponentId::new).collect(),
            sender_link_id: ComponentId::new(self.num_senders),
            receiver_id: ComponentId::new(self.num_senders + 1),
            receiver_link_id: ComponentId::new(self.num_senders + 2),
        }
    }

    #[must_use]
    pub fn to_sim<'a, E, P>(
        &self,
        rng: &'a mut Rng,
        senders: Vec<DynComponent<'a, E>>,
        receiver: DynComponent<'a, E>,
    ) -> Simulator<'a, E, NothingLogger>
    where
        E: HasVariant<P> + 'a,
        P: Routable + 'a,
    {
        let mut components: Vec<_> = senders;
        components.extend([
            DynComponent::new(Link::create(
                0.5 * self.rtt,
                self.packet_rate,
                self.loss_rate,
                self.buffer_size,
                NothingLogger,
            )),
            receiver,
            DynComponent::new(Link::create(
                0.5 * self.rtt,
                self.packet_rate,
                self.loss_rate,
                self.buffer_size,
                NothingLogger,
            )),
        ]);
        Simulator::<E, _>::new(components, rng, NothingLogger)
    }
}
