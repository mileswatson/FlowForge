use std::collections::VecDeque;

use derive_where::derive_where;

use crate::{
    logging::Logger,
    quantities::{earliest_opt, latest, Information, InformationRate, Time, TimeSpan},
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, EffectContext, Message},
};

use super::Packet;

#[derive_where(Debug; L)]
pub struct Link<'sim, E, L> {
    delay: TimeSpan,
    packet_rate: InformationRate,
    loss: f64,
    buffer_size: Option<Information>,
    earliest_transmit: Time,
    buffer_contains: Information,
    buffer: VecDeque<Packet<'sim, E>>,
    transmitting: VecDeque<(Packet<'sim, E>, Time)>,
    rng: Rng,
    logger: L,
}

impl<'sim, 'a, E, L> Link<'sim, E, L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create(
        delay: TimeSpan,
        packet_rate: InformationRate,
        loss: f64,
        buffer_size: Option<Information>,
        rng: Rng,
        logger: L,
    ) -> Self {
        Link {
            delay,
            packet_rate,
            loss,
            buffer_size,
            earliest_transmit: Time::MIN,
            buffer_contains: Information::ZERO,
            buffer: VecDeque::new(),
            transmitting: VecDeque::new(),
            rng,
            logger,
        }
    }
}

impl<'sim, E, L> Link<'sim, E, L>
where
    L: Logger,
{
    fn try_transmit(&mut self, time: Time) {
        // If there is a planned buffer release then wait for it
        if time < self.earliest_transmit {
            return;
        }

        if let Some(p) = self.buffer.pop_front() {
            // Don't transmit another packet until this time
            self.earliest_transmit = time + p.size() / self.packet_rate;
            self.buffer_contains = self.buffer_contains - p.size();
            self.transmitting.push_back((p, time + self.delay));
        }
    }

    #[must_use]
    fn try_deliver(&mut self, time: Time) -> Option<Message<'sim, E>> {
        match self.transmitting.front() {
            Some((_, t)) if t == &time => {
                let (mut packet, _) = self.transmitting.pop_front().unwrap();
                // Randomly drop packets to simulate loss
                if self
                    .rng
                    .sample(&ContinuousDistribution::Uniform { min: 0., max: 1. })
                    < self.loss
                {
                    log!(self.logger, "Dropped packet (loss)");
                    None
                } else {
                    let next_hop = packet.pop_next_hop();
                    Some(next_hop.create_message(packet))
                }
            }
            _ => None,
        }
    }
}

impl<'sim, E, L> Component<'sim, E> for Link<'sim, E, L>
where
    L: Logger,
{
    type Receive = Packet<'sim, E>;

    fn tick(&mut self, EffectContext { time, .. }: EffectContext) -> Vec<Message<'sim, E>> {
        assert_eq!(Some(time), Component::next_tick(self, time));
        let mut effects = Vec::new();
        if let Some(msg) = self.try_deliver(time) {
            effects.push(msg);
        }
        self.try_transmit(time);
        effects
    }

    fn receive(&mut self, packet: Self::Receive, _ctx: EffectContext) -> Vec<Message<'sim, E>> {
        if self
            .buffer_size
            .is_some_and(|limit| self.buffer_contains + packet.size() > limit)
        {
            log!(self.logger, "Dropped packet (buffer full)");
        } else {
            log!(self.logger, "Buffered packet");
            self.buffer_contains = self.buffer_contains + packet.size();
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
