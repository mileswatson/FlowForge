use std::fmt::Debug;

use crate::{
    logging::Logger,
    rand::ContinuousDistribution,
    simulation::{Component, EffectContext, EffectResult, EventQueue, HasVariant, Message},
};

pub trait Routable {
    fn pop_next_hop(&mut self) -> usize;
}

#[derive(Debug)]
pub struct Link<P, L> {
    delay: f64,
    loss: f64,
    received_count: u64,
    to_deliver: EventQueue<u64, P>,
    logger: L,
}

impl<'a, P, L> Link<P, L>
where
    L: Logger + 'a,
    P: Routable + 'a,
{
    #[must_use]
    pub fn create<E>(delay: f64, loss: f64, logger: L) -> Box<dyn Component<E> + 'a>
    where
        E: 'a + HasVariant<P>,
    {
        Box::new(Link {
            delay,
            loss,
            received_count: 0,
            to_deliver: EventQueue::new(),
            logger,
        })
    }
}

impl<P, L> Link<P, L> {
    fn no_effects<E>(&self) -> EffectResult<E> {
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![],
        }
    }

    fn effects<E>(&self, effects: Vec<Message<E>>) -> EffectResult<E> {
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects,
        }
    }
}

impl<E, P, L> Component<E> for Link<P, L>
where
    L: Logger,
    E: HasVariant<P>,
    P: Routable,
{
    fn tick(&mut self, _: EffectContext) -> EffectResult<E> {
        let mut packet = match self.to_deliver.pop_next() {
            Some(x) => x.2,
            None => return self.no_effects(),
        };
        log!(self.logger, "Delivered packet");
        let next_hop = packet.pop_next_hop();
        self.effects(vec![Message::new(next_hop, packet)])
    }

    fn receive(
        &mut self,
        effect: E,
        EffectContext { time, rng, .. }: EffectContext,
    ) -> EffectResult<E> {
        let packet = HasVariant::<P>::try_into(effect).unwrap();
        if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
            log!(self.logger, "Dropped packet");
        } else {
            self.to_deliver
                .insert_or_update(self.received_count, packet, Some(time + self.delay));
            self.received_count += 1;
        }
        self.no_effects()
    }
}
