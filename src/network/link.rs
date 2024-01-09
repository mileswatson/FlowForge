use std::{collections::VecDeque, fmt::Debug};

use uom::{
    si::{
        f64::{InformationRate, Time},
        u64::Information,
    },
    ConstZero,
};

use crate::{
    logging::Logger,
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, EffectContext, MaybeHasVariant, Message},
    time::{earliest_opt, latest, transmission_time, TimePoint},
};

use super::{NetworkEffect, NetworkMessage, Packet};

#[derive(Debug)]
pub struct Link<L> {
    delay: Time,
    packet_rate: InformationRate,
    loss: f64,
    buffer_size: Option<Information>,
    earliest_transmit: TimePoint,
    buffer_contains: Information,
    buffer: VecDeque<Packet>,
    transmitting: VecDeque<(Packet, TimePoint)>,
    logger: L,
}

impl<'a, L> Link<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create(
        delay: Time,
        packet_rate: InformationRate,
        loss: f64,
        buffer_size: Option<Information>,
        logger: L,
    ) -> Self {
        Link {
            delay,
            packet_rate,
            loss,
            buffer_contains: Information::ZERO,
            buffer_size,
            earliest_transmit: TimePoint::min(),
            buffer: VecDeque::new(),
            transmitting: VecDeque::new(),
            logger,
        }
    }
}

impl<L> Link<L>
where
    L: Logger,
{
    fn try_transmit(&mut self, time: TimePoint) {
        // If there is a planned buffer release then wait for it
        if time < self.earliest_transmit {
            return;
        }

        if let Some(p) = self.buffer.pop_front() {
            self.buffer_contains -= Packet::size();
            self.transmitting.push_back((p, time + self.delay));
            // Don't transmit another packet until this time
            self.earliest_transmit = time + transmission_time(Packet::size(), self.packet_rate);
        }
    }

    #[must_use]
    fn try_deliver(&mut self, time: TimePoint, rng: &mut Rng) -> Option<NetworkMessage> {
        match self.transmitting.front() {
            Some((_, t)) if t == &time => {
                let (mut pkt, _) = self.transmitting.pop_front().unwrap();
                // Randomly drop packets to simulate loss
                if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
                    log!(self.logger, "Dropped packet (loss)");
                    None
                } else {
                    let next_hop = pkt.pop_next_hop();
                    Some(Message::new(next_hop, pkt))
                }
            }
            _ => None,
        }
    }
}

impl<L> Component<NetworkEffect> for Link<L>
where
    L: Logger,
{
    fn tick(&mut self, EffectContext { time, rng, .. }: EffectContext) -> Vec<NetworkMessage> {
        assert_eq!(Some(time), Component::next_tick(self, time));
        let mut effects = Vec::new();
        if let Some(msg) = self.try_deliver(time, rng) {
            effects.push(msg);
        }
        self.try_transmit(time);
        effects
    }

    fn receive(&mut self, effect: NetworkEffect, _ctx: EffectContext) -> Vec<NetworkMessage> {
        let pkt: Packet = MaybeHasVariant::try_into(effect).unwrap();
        if self
            .buffer_size
            .is_some_and(|limit| self.buffer_contains + Packet::size() > limit)
        {
            log!(self.logger, "Dropped packet (buffer full)");
        } else {
            log!(self.logger, "Buffered packet");
            self.buffer_contains += Packet::size();
            self.buffer.push_back(pkt);
        }
        vec![]
    }

    fn next_tick(&self, time: TimePoint) -> Option<TimePoint> {
        let next_try_transmit = if self.buffer.is_empty() {
            None
        } else {
            Some(latest(&[time, self.earliest_transmit]))
        };
        let next_try_deliver = self.transmitting.front().map(|x| x.1);
        earliest_opt(&[next_try_transmit, next_try_deliver])
    }
}
