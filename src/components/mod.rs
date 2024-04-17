use derive_where::derive_where;

use crate::{
    quantities::{packets, Information, Time},
    simulation::Address,
};

pub mod bouncer;
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
