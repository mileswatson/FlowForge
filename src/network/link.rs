use std::fmt::Debug;

use crate::{
    rand::Rng,
    simulation::{Component, EffectResult, EventQueue, HasVariant, Message, Time},
};

pub struct Link<P> {
    destination: usize,
    delay: f64,
    received_count: u64,
    to_deliver: EventQueue<u64, P>,
}

impl<P> Link<P>
where
    P: 'static,
{
    #[must_use]
    pub fn create<E>(destination: usize, delay: f64) -> Box<dyn Component<E>>
    where
        E: Debug + HasVariant<P>,
    {
        Box::new(Link {
            destination,
            delay,
            received_count: 0,
            to_deliver: EventQueue::new(),
        })
    }
}

impl<P, E> Component<E> for Link<P>
where
    E: Debug + HasVariant<P>,
{
    fn tick(&mut self, _: Time, _: &mut Rng) -> EffectResult<E> {
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
        let packet = effect.try_into().unwrap();
        self.to_deliver
            .insert_or_update(self.received_count, packet, Some(time + self.delay));
        self.received_count += 1;
        EffectResult {
            next_tick: self.to_deliver.next_time(),
            effects: vec![],
        }
    }
}
