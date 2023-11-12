use std::{collections::VecDeque, fmt::Debug};

use crate::{
    logging::Logger,
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, EffectContext, EffectResult, EventQueue, HasVariant, Message},
    time::{earliest, Rate, Time, TimeSpan},
};

pub trait Routable {
    fn pop_next_hop(&mut self) -> usize;
}

#[derive(Debug)]
pub struct Link<P, L> {
    delay: TimeSpan,
    packet_rate: Rate,
    loss: f64,
    buffer_size: Option<usize>,
    received_count: u64,
    next_dispatch: Option<Time>,
    buffer: VecDeque<P>,
    to_deliver: EventQueue<u64, P>,
    logger: L,
}

impl<'a, P, L> Link<P, L>
where
    L: Logger + 'a,
    P: Routable + 'a,
{
    #[must_use]
    pub fn create<E>(
        delay: TimeSpan,
        packet_rate: Rate,
        loss: f64,
        buffer_size: Option<usize>,
        logger: L,
    ) -> Box<dyn Component<E> + 'a>
    where
        E: 'a + HasVariant<P>,
    {
        Box::new(Link {
            delay,
            packet_rate,
            loss,
            buffer_size,
            received_count: 0,
            next_dispatch: None,
            buffer: VecDeque::new(),
            to_deliver: EventQueue::new(),
            logger,
        })
    }
}

impl<P, L> Link<P, L>
where
    L: Logger,
    P: Routable,
{
    fn next_tick(&self) -> Option<Time> {
        earliest(&[self.to_deliver.next_time(), self.next_dispatch])
    }

    fn no_effects<E>(&self) -> EffectResult<E> {
        EffectResult {
            next_tick: self.next_tick(),
            effects: vec![],
        }
    }

    fn effects<E>(&self, effects: Vec<Message<E>>) -> EffectResult<E> {
        EffectResult {
            next_tick: self.next_tick(),
            effects,
        }
    }

    fn try_dispatch(&mut self, time: Time, rng: &mut Rng) {
        // If there is a planned buffer release then wait for it
        if self.next_dispatch.map_or(false, |t| t != time) {
            return;
        }

        if let Some(packet) = self.buffer.pop_front() {
            // Randomly drop packets to simulate loss
            if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
                log!(self.logger, "Dropped packet (loss)");
            } else {
                log!(self.logger, "Dispatched packet");
                self.to_deliver.insert_or_update(
                    self.received_count,
                    packet,
                    Some(time + self.delay),
                );
                self.received_count += 1;
            }
            // Don't dispatch another packet until this time
            self.next_dispatch = Some(time + self.packet_rate.period());
        } else {
            // No packets in the buffer, so next one can dispatch immediately
            self.next_dispatch = None;
        }
    }

    #[must_use]
    fn try_deliver<E>(&mut self, time: Time) -> Option<Message<E>>
    where
        E: HasVariant<P>,
    {
        if Some(time) == self.to_deliver.next_time() {
            let mut packet = match self.to_deliver.pop_next() {
                Some(x) => x.2,
                None => return None,
            };
            log!(self.logger, "Delivered packet");
            let next_hop = packet.pop_next_hop();
            Some(Message::new(next_hop, packet))
        } else {
            None
        }
    }
}

impl<E, P, L> Component<E> for Link<P, L>
where
    L: Logger,
    E: HasVariant<P>,
    P: Routable,
{
    fn tick(&mut self, EffectContext { time, rng, .. }: EffectContext) -> EffectResult<E> {
        assert!(self
            .next_tick()
            .map_or(time == Time::sim_start(), |t| time == t));
        let mut effects = Vec::new();
        if let Some(msg) = self.try_deliver::<E>(time) {
            effects.push(msg);
        }
        self.try_dispatch(time, rng);
        self.effects(effects)
    }

    fn receive(
        &mut self,
        effect: E,
        EffectContext { time, rng, .. }: EffectContext,
    ) -> EffectResult<E> {
        let packet = HasVariant::<P>::try_into(effect).unwrap();
        if self
            .buffer_size
            .is_some_and(|limit| self.buffer.len() == limit)
        {
            log!(self.logger, "Dropped packet (buffer full)");
        } else {
            log!(self.logger, "Buffered packet");
            self.buffer.push_back(packet);
            self.try_dispatch(time, rng);
        }
        self.no_effects()
    }
}
