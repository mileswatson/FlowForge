use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use ordered_float::NotNan;
use priority_queue::PriorityQueue;

use crate::{logging::Logger, rand::Rng};

pub type Time = f64;

pub trait HasVariant<T>: From<T> + Debug {
    fn try_into(self) -> Result<T, Self>;
}

pub struct Message<E> {
    pub component_index: usize,
    pub effect: E,
}

impl<E> Message<E> {
    pub fn new<V>(component_index: usize, effect: V) -> Message<E>
    where
        E: From<V>,
    {
        Message {
            component_index,
            effect: E::from(effect),
        }
    }
}

pub struct EffectResult<E> {
    pub next_tick: Option<Time>,
    pub effects: Vec<Message<E>>,
}

pub trait Component<'a, E> {
    fn tick(&mut self, time: Time, rng: &mut Rng) -> EffectResult<E>;
    fn receive(&mut self, e: E, time: Time, rng: &mut Rng) -> EffectResult<E>;
}

#[derive(Debug)]
pub struct EventQueue<I: Hash + Eq, E> {
    current_time: Time,
    waiting: HashMap<I, E>,
    queue: PriorityQueue<I, Reverse<NotNan<Time>>>,
}

impl<I: Hash + Eq + Copy, E> EventQueue<I, E> {
    #[must_use]
    pub fn new() -> EventQueue<I, E> {
        EventQueue {
            current_time: 0.,
            waiting: HashMap::new(),
            queue: PriorityQueue::new(),
        }
    }

    pub fn update(&mut self, id: I, time: Option<Time>) {
        if let Some(time) = time {
            assert!(time >= self.current_time);
            self.queue.push(id, Reverse(NotNan::new(time).unwrap()));
        } else {
            self.waiting.remove(&id);
            self.queue.remove(&id);
        }
    }

    pub fn insert_or_update(&mut self, id: I, event: E, time: Option<Time>) {
        self.waiting.insert(id, event);
        self.update(id, time);
    }

    #[must_use]
    pub fn next_time(&self) -> Option<Time> {
        self.queue.peek().map(|(_, Reverse(x))| **x)
    }

    pub fn pop_next(&mut self) -> Option<(Time, I, E)> {
        if let Some((component_index, Reverse(time))) = self.queue.pop() {
            self.current_time = *time;
            Some((
                *time,
                component_index,
                self.waiting.remove(&component_index).unwrap(),
            ))
        } else {
            None
        }
    }
}

impl<I: Hash + Eq + Copy, E> Default for EventQueue<I, E> {
    fn default() -> Self {
        Self::new()
    }
}

struct EffectQueue<E> {
    queue: VecDeque<Message<E>>,
}

impl<E> EffectQueue<E> {
    const fn new() -> EffectQueue<E> {
        EffectQueue {
            queue: VecDeque::new(),
        }
    }

    fn push_all<T: IntoIterator<Item = Message<E>>>(&mut self, effects: T) {
        self.queue.extend(effects);
    }

    fn pop_next(&mut self) -> Option<Message<E>> {
        self.queue.pop_front()
    }
}

pub struct Simulator<'a, E, L> {
    components: Vec<Box<dyn Component<'a, E>>>,
    rng: Rng,
    tick_queue: EventQueue<usize, ()>,
    logger: L,
}

impl<'a, E, L> Simulator<'a, E, L>
where
    L: Logger,
{
    #[must_use]
    pub fn new(components: Vec<Box<dyn Component<E>>>, rng: Rng, logger: L) -> Simulator<E, L> {
        Simulator {
            components,
            rng,
            tick_queue: EventQueue::new(),
            logger,
        }
    }

    fn handle_effects(&mut self, time: Time, effects: &mut EffectQueue<E>) {
        while let Some(Message {
            component_index,
            effect,
        }) = effects.pop_next()
        {
            let EffectResult {
                next_tick,
                effects: signals,
            } = self.components[component_index].receive(effect, time, &mut self.rng);
            self.tick_queue
                .insert_or_update(component_index, (), next_tick);
            effects.push_all(signals);
        }
    }

    fn tick_without_effects(
        &mut self,
        component_index: usize,
        time: Time,
        effects: &mut EffectQueue<E>,
    ) {
        let EffectResult {
            next_tick,
            effects: signals,
        } = self.components[component_index].tick(time, &mut self.rng);
        self.tick_queue
            .insert_or_update(component_index, (), next_tick);
        effects.push_all(signals);
    }

    fn first_tick(&mut self) {
        self.logger.log("time = 0.0");
        let mut effects = EffectQueue::new();
        for i in 0..self.components.len() {
            self.tick_without_effects(i, 0., &mut effects);
        }
        self.handle_effects(0., &mut effects);
    }

    fn tick(&mut self, component_index: usize, time: Time) {
        self.logger.log(&format!("time = {}", &time));
        let mut effects = EffectQueue::new();
        self.tick_without_effects(component_index, time, &mut effects);
        self.handle_effects(time, &mut effects);
    }

    pub fn run_until(mut self, end_time: f64) {
        self.first_tick();
        while let Some((time, component_index, ())) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(component_index, time);
        }
    }
}
