use std::{collections::VecDeque, fmt::Debug};

use crate::{
    logging::Logger,
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, EffectContext, MaybeHasVariant, Message},
    time::{earliest_opt, latest, Rate, Time, TimeSpan},
};

use super::{NetworkEffect, NetworkMessage, Packet};

#[derive(Debug)]
pub struct Link<L> {
    delay: TimeSpan,
    packet_rate: Rate,
    loss: f64,
    buffer_size: Option<usize>,
    earliest_transmit: Time,
    buffer: VecDeque<Packet>,
    transmitting: VecDeque<(Packet, Time)>,
    logger: L,
}

impl<'a, L> Link<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create(
        delay: TimeSpan,
        packet_rate: Rate,
        loss: f64,
        buffer_size: Option<usize>,
        logger: L,
    ) -> Self {
        Link {
            delay,
            packet_rate,
            loss,
            buffer_size,
            earliest_transmit: Time::MIN,
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
    fn try_transmit(&mut self, time: Time) {
        // If there is a planned buffer release then wait for it
        if time < self.earliest_transmit {
            return;
        }

        if let Some(p) = self.buffer.pop_front() {
            self.transmitting.push_back((p, time + self.delay));
            // Don't transmit another packet until this time
            self.earliest_transmit = time + self.packet_rate.period();
        }
    }

    #[must_use]
    fn try_deliver(&mut self, time: Time, rng: &mut Rng) -> Option<NetworkMessage> {
        match self.transmitting.front() {
            Some((_, t)) if t == &time => {
                let (mut packet, _) = self.transmitting.pop_front().unwrap();
                // Randomly drop packets to simulate loss
                if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
                    log!(self.logger, "Dropped packet (loss)");
                    None
                } else {
                    let next_hop = packet.pop_next_hop();
                    Some(Message::new(next_hop, packet))
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
        let packet = MaybeHasVariant::try_into(effect).unwrap();
        if self
            .buffer_size
            .is_some_and(|limit| self.buffer.len() == limit)
        {
            log!(self.logger, "Dropped packet (buffer full)");
        } else {
            log!(self.logger, "Buffered packet");
            self.buffer.push_back(packet);
        }
        vec![]
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        let next_try_transmit = if self.buffer.is_empty() {
            None
        } else {
            Some(latest(&[time, self.earliest_transmit]))
        };
        let next_try_deliver = self.transmitting.front().map(|x| x.1);
        earliest_opt(&[next_try_transmit, next_try_deliver])
    }
}
