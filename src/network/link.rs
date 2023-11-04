use std::fmt::Debug;

use crate::{
    logging::Logger,
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, EffectResult, EventQueue, Message, Time},
};

#[derive(Debug)]
pub struct Link<E, L> {
    destination: usize,
    delay: f64,
    loss: f64,
    received_count: u64,
    to_deliver: EventQueue<u64, E>,
    logger: L,
}

impl<'a, E, L> Link<E, L>
where
    E: 'static,
    L: Logger + 'a,
{
    #[must_use]
    pub fn create(
        destination: usize,
        delay: f64,
        loss: f64,
        logger: L,
    ) -> Box<dyn Component<E> + 'a> {
        Box::new(Link {
            destination,
            delay,
            loss,
            received_count: 0,
            to_deliver: EventQueue::new(),
            logger,
        })
    }
}

impl<E, L> Link<E, L> {
    fn effect_result(&self, effects: Vec<Message<E>>) -> EffectResult<E> {
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects,
        }
    }
}

impl<E, L> Component<E> for Link<E, L>
where
    L: Logger,
{
    fn tick(&mut self, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let packet = match self.to_deliver.pop_next() {
            Some(x) => x.2,
            None => return self.effect_result(vec![]),
        };
        self.logger.log("Delivered packet");
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![Message::new(self.destination, packet)],
        }
    }

    fn receive(&mut self, effect: E, time: Time, rng: &mut Rng) -> EffectResult<E> {
        if rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 1. }) < self.loss {
            self.logger.log("Dropped packet");
        } else {
            self.to_deliver
                .insert_or_update(self.received_count, effect, Some(time + self.delay));
            self.received_count += 1;
        }
        self.effect_result(vec![])
    }
}
