use std::fmt::Debug;

use crate::{
    rand::Rng,
    simulation::{Component, EffectResult, EventQueue, Message, Time},
};

#[derive(Debug)]
pub struct Link<E> {
    destination: usize,
    delay: f64,
    received_count: u64,
    to_deliver: EventQueue<u64, E>,
}

impl<E> Link<E>
where
    E: 'static,
{
    #[must_use]
    pub fn create(destination: usize, delay: f64) -> Box<dyn Component<E>> {
        Box::new(Link {
            destination,
            delay,
            received_count: 0,
            to_deliver: EventQueue::new(),
        })
    }
}

impl<E> Component<E> for Link<E> {
    fn tick(&mut self, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let packet = match self.to_deliver.pop_next() {
            Some(x) => x.2,
            None => {
                return EffectResult {
                    next_tick: self.to_deliver.next_time(),
                    effects: vec![],
                }
            }
        };
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![Message::new(self.destination, packet)],
        }
    }

    fn receive(&mut self, effect: E, time: Time, _rng: &mut Rng) -> EffectResult<E> {
        self.to_deliver
            .insert_or_update(self.received_count, effect, Some(time + self.delay));
        self.received_count += 1;
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![],
        }
    }
}
