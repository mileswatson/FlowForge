use std::{collections::VecDeque, fmt::Debug};

use crate::{
    logging::Logger,
    rand::{ContinuousDistribution, Rng},
    simulation::{
        Component, ComponentId, EffectContext, EffectResult, HasVariant, Message,
    },
    time::{earliest_opt, Rate, Time, TimeSpan},
};

pub trait Routable: Sync + 'static {
    fn pop_next_hop(&mut self) -> ComponentId;
}

#[derive(Debug)]
pub struct Link<P, L> {
    delay: TimeSpan,
    packet_rate: Rate,
    loss: f64,
    buffer_size: Option<usize>,
    next_transmit: Option<Time>,
    buffer: VecDeque<P>,
    transmitting: VecDeque<(P, Time)>,
    logger: L,
}

impl<'a, P, L> Link<P, L>
where
    L: Logger + 'a,
    P: Routable + 'a,
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
            next_transmit: None,
            buffer: VecDeque::new(),
            transmitting: VecDeque::new(),
            logger,
        }
    }
}

impl<P, L> Link<P, L>
where
    L: Logger,
    P: Routable,
{
    fn next_tick(&self) -> Option<Time> {
        earliest_opt(&[self.next_transmit, self.transmitting.front().map(|x| x.1)])
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

    fn try_transmit(&mut self, time: Time) {
        // If there is a planned buffer release then wait for it
        if self.next_transmit.map_or(false, |t| t != time) {
            return;
        }

        match self.buffer.pop_front() {
            Some(p) => {
                self.transmitting.push_back((p, time + self.delay));
                // Don't transmit another packet until this time
                self.next_transmit = Some(time + self.packet_rate.period());
            }
            None => {
                // No packets in the buffer, so next one can transmit immediately
                self.next_transmit = None;
            }
        }
    }

    #[must_use]
    fn try_deliver<E>(&mut self, time: Time, rng: &mut Rng) -> Option<Message<E>>
    where
        E: HasVariant<P>,
    {
        match self.transmitting.front() {
            Some((_,t)) if t == &time => {
                let (mut packet, _) = self.transmitting.pop_front().unwrap();
                // Randomly drop packets to simulate loss
                if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
                    log!(self.logger, "Dropped packet (loss)");
                    None
                } else {
                    log!(self.logger, "Delivered packet");
                    let next_hop = packet.pop_next_hop();
                    Some(Message::new(next_hop, packet))
                }
            },
            _ => None,
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
        let mut effects = Vec::new();
        if let Some(msg) = self.try_deliver::<E>(time, rng) {
            effects.push(msg);
        }
        self.try_transmit(time);
        self.effects(effects)
    }

    fn receive(
        &mut self,
        effect: E,
        ctx: EffectContext,
    ) -> EffectResult<E> {
        let packet = HasVariant::<P>::try_into(effect).unwrap();
        if self
            .buffer_size
            .is_some_and(|limit| self.buffer.len() == limit)
        {
            log!(self.logger, "Dropped packet (buffer full)");
            self.no_effects()
        } else {
            log!(self.logger, "Buffered packet");
            self.buffer.push_back(packet);
            self.tick(ctx)
        }
    }
}
