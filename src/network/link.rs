use std::collections::HashMap;

use crate::{
    rand::Rng,
    simulation::{Component, Effect, EffectResult, EventQueue, Time},
};

use super::CanReceive;

pub struct Link<P> {
    destination: usize,
    delay: f64,
    received_count: u64,
    waiting: HashMap<u64, P>,
    to_deliver: EventQueue<u64>,
}

impl<P: 'static> Link<P> {
    #[must_use]
    pub fn create(destination: usize, delay: f64) -> Box<dyn Component> {
        Box::new(Link {
            destination,
            delay,
            received_count: 0,
            waiting: HashMap::<u64, P>::new(),
            to_deliver: EventQueue::new(),
        })
    }
}

impl<P: 'static> CanReceive<P> for Link<P> {
    fn receive(&mut self, time: Time, packet: P, _: &mut Rng) -> EffectResult {
        self.waiting.insert(self.received_count, packet);
        self.to_deliver
            .set(self.received_count, Some(time + self.delay));
        self.received_count += 1;
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![],
        }
    }
}

impl<P: 'static> Component for Link<P> {
    fn tick(&mut self, _: Time, _: &mut Rng) -> EffectResult {
        let id = match self.to_deliver.pop_next() {
            Some(x) => x.1,
            None => {
                return EffectResult {
                    next_tick: self.to_deliver.next_time(),
                    effects: vec![],
                }
            }
        };
        let packet = self.waiting.remove(&id).unwrap();
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![Effect::new(
                self.destination,
                move |c: &mut dyn CanReceive<P>, time, rng| c.receive(time, packet, rng),
            )],
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
