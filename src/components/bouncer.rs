use derive_where::derive_where;

use crate::{
    core::logging::Logger,
    quantities::Time,
    simulation::{Component, EffectContext, Message},
};

use super::packet::{Packet, PacketAddress};

#[derive_where(Debug; L)]
pub struct LossyBouncer<'sim, E, L> {
    link: PacketAddress<'sim, E>,
    logger: L,
}

impl<'sim, E, L> LossyBouncer<'sim, E, L> {
    pub const fn new(link: PacketAddress<'sim, E>, logger: L) -> LossyBouncer<E, L> {
        LossyBouncer { link, logger }
    }
}

impl<'sim, E, L> Component<'sim, E> for LossyBouncer<'sim, E, L>
where
    L: Logger,
{
    type Receive = Packet<'sim, E>;

    fn tick(&mut self, _: EffectContext) -> Vec<Message<'sim, E>> {
        vec![]
    }

    fn receive(&mut self, packet: Self::Receive, _: EffectContext) -> Vec<Message<'sim, E>> {
        let seq = packet.seq;
        let message = self.link.create_message(Packet {
            source: packet.destination,
            destination: packet.source,
            ..packet
        });
        log!(
            self.logger,
            "Bouncing packet {} via {:?}",
            seq,
            message.destination()
        );
        vec![message]
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        None
    }
}
