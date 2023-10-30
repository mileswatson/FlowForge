use std::{any::Any, cmp::Reverse, collections::VecDeque, hash::Hash};

use ordered_float::NotNan;
use priority_queue::PriorityQueue;

use crate::rand::Rng;

pub type Time = f64;
pub type EffectFn = dyn FnOnce(&mut ComponentWrapper, Time, &mut Rng) -> EffectResult;

pub struct Effect {
    pub component_index: usize,
    pub effect: Box<EffectFn>,
}

impl Effect {
    pub fn new<C>(
        component_index: usize,
        effect: impl FnOnce(&mut C, Time, &mut Rng) -> EffectResult + 'static,
    ) -> Effect
    where
        C: Component + ?Sized,
    {
        Effect {
            component_index,
            effect: Box::new(|c: &mut ComponentWrapper, time, rng| {
                effect(c.downcast::<C>(), time, rng)
            }),
        }
    }
}

pub struct EffectResult {
    pub next_tick: Option<Time>,
    pub effects: Vec<Effect>,
}

pub trait Component: Any {
    fn tick(&mut self, time: Time, rng: &mut Rng) -> EffectResult;
    fn as_any(&mut self) -> &mut dyn Any;
}

pub struct ComponentWrapper {
    component: Box<dyn Component>,
}

impl ComponentWrapper {
    fn new(component: Box<dyn Component>) -> ComponentWrapper {
        ComponentWrapper { component }
    }

    /// Attempts to downcast into an particular implementation of [`Component`].
    ///
    /// # Panics
    ///
    /// Panics if the underlying type of the component isn't `C`.
    pub fn downcast<C: Component + ?Sized>(&mut self) -> &mut C {
        self.component
            .as_any()
            .downcast_mut::<Box<&mut C>>()
            .unwrap()
    }

    fn as_component(&mut self) -> &mut dyn Component {
        &mut *self.component
    }
}

pub struct EventQueue<E: Hash + Eq> {
    current_time: Time,
    queue: PriorityQueue<E, Reverse<NotNan<Time>>>,
}

impl<E: Hash + Eq> EventQueue<E> {
    #[must_use]
    pub fn new() -> EventQueue<E> {
        EventQueue {
            current_time: 0.,
            queue: PriorityQueue::new(),
        }
    }

    /// Updates the timing of the event if `time` is `Some`, otherwise removes it.
    ///
    /// # Panics
    ///
    /// Panics if the provided time is before the time of the last-popped event (or 0, if no events were popped).
    pub fn set(&mut self, event: E, time: Option<Time>) {
        match time {
            Some(time) => {
                assert!(time >= self.current_time);
                self.queue.push(event, Reverse(NotNan::new(time).unwrap()));
            }
            None => {
                self.queue.remove(&event);
            }
        }
    }

    #[must_use]
    pub fn next_time(&self) -> Option<Time> {
        self.queue.peek().map(|(_, Reverse(x))| **x)
    }

    pub fn pop_next(&mut self) -> Option<(Time, E)> {
        if let Some((component_index, Reverse(time))) = self.queue.pop() {
            self.current_time = *time;
            Some((*time, component_index))
        } else {
            None
        }
    }
}

impl<E: Hash + Eq> Default for EventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

struct EffectQueue {
    queue: VecDeque<Effect>,
}

impl EffectQueue {
    const fn new() -> EffectQueue {
        EffectQueue {
            queue: VecDeque::new(),
        }
    }

    fn push_all<T: IntoIterator<Item = Effect>>(&mut self, effects: T) {
        self.queue.extend(effects);
    }

    fn pop_next(&mut self) -> Option<Effect> {
        self.queue.pop_front()
    }
}

pub struct Simulator {
    components: Vec<ComponentWrapper>,
    rng: Rng,
    tick_queue: EventQueue<usize>,
}

impl Simulator {
    pub fn new(components: Vec<Box<dyn Component>>, rng: Rng) -> Simulator {
        Simulator {
            components: components.into_iter().map(ComponentWrapper::new).collect(),
            rng,
            tick_queue: EventQueue::new(),
        }
    }

    fn handle_effects(&mut self, time: Time, effects: &mut EffectQueue) {
        while let Some(Effect {
            component_index,
            effect,
        }) = effects.pop_next()
        {
            let EffectResult {
                next_tick,
                effects: signals,
            } = effect(&mut self.components[component_index], time, &mut self.rng);
            self.tick_queue.set(component_index, next_tick);
            effects.push_all(signals);
        }
    }

    fn tick_without_effects(
        &mut self,
        component_index: usize,
        time: Time,
        effects: &mut EffectQueue,
    ) {
        let EffectResult {
            next_tick,
            effects: signals,
        } = self.components[component_index]
            .as_component()
            .tick(time, &mut self.rng);
        self.tick_queue.set(component_index, next_tick);
        effects.push_all(signals);
    }

    fn first_tick(&mut self) {
        let mut effects = EffectQueue::new();
        for i in 0..self.components.len() {
            self.tick_without_effects(i, 0., &mut effects);
        }
        self.handle_effects(0., &mut effects);
    }

    fn tick(&mut self, component_index: usize, time: Time) {
        let mut effects = EffectQueue::new();
        self.tick_without_effects(component_index, time, &mut effects);
        self.handle_effects(time, &mut effects);
    }

    pub fn run(mut self) {
        self.first_tick();
        while let Some((time, component_index)) = self.tick_queue.pop_next() {
            self.tick(component_index, time);
        }
    }
}
