use derive_where::derive_where;

use crate::{
    quantities::{packets, Information, Time},
    simulation::Address,
};

#[derive_where(Debug)]
pub struct Packet<'sim, E> {
    pub(super) seq: u64,
    pub(super) source: Address<'sim, Packet<'sim, E>, E>,
    pub(super) destination: Address<'sim, Packet<'sim, E>, E>,
    pub(super) sent_time: Time,
}

impl<'sim, E> Packet<'sim, E> {
    pub fn pop_next_hop(&mut self) -> Address<'sim, Packet<'sim, E>, E> {
        self.destination.clone()
    }

    #[allow(clippy::unused_self)]
    #[must_use]
    pub const fn size(&self) -> Information {
        packets(1)
    }
}

pub type PacketAddress<'sim, E> = Address<'sim, Packet<'sim, E>, E>;
