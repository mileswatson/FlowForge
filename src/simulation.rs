use std::{any::Any, cmp::Reverse, collections::VecDeque};

use ordered_float::NotNan;
use priority_queue::PriorityQueue;

use crate::rand::Rng;

pub type Time = f64;
pub type EffectFn = dyn FnOnce(&mut ComponentWrapper, &mut Rng) -> EffectResult;

pub struct Effect {
    component_index: usize,
    effect: Box<EffectFn>,
}

pub struct EffectResult {
    next_tick: Time,
    effects: Vec<Effect>,
}

pub trait Component: Any {
    fn tick(&self, time: Time, rng: &mut Rng) -> EffectResult;
    fn as_any(&mut self) -> &mut dyn Any;
}

pub struct ComponentWrapper {
    component: Box<dyn Component>,
}

impl ComponentWrapper {
    fn new(component: Box<dyn Component>) -> Self {
        Self { component }
    }

    pub fn assert_is<C: Component>(&mut self) -> &mut C {
        self.component.as_any().downcast_mut().unwrap()
    }

    fn as_component(&mut self) -> &dyn Component {
        &*self.component
    }
}

struct Tick {
    pub component_index: usize,
    pub time: Time,
}

struct TickQueue {
    current_time: Time,
    queue: PriorityQueue<usize, Reverse<NotNan<Time>>>,
}

impl TickQueue {
    pub fn new() -> Self {
        Self {
            current_time: 0.,
            queue: PriorityQueue::new(),
        }
    }

    pub fn add_or_update(&mut self, component_index: usize, time: Time) {
        assert!(time <= self.current_time);
        self.queue
            .push(component_index, Reverse(NotNan::new(time).unwrap()));
    }

    pub fn next(&mut self) -> Option<Tick> {
        if let Some((component_index, Reverse(time))) = self.queue.pop() {
            self.current_time = *time;
            Some(Tick {
                component_index,
                time: *time,
            })
        } else {
            None
        }
    }
}

struct EffectQueue {
    queue: VecDeque<Effect>,
}

impl EffectQueue {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    fn push_all<T: IntoIterator<Item = Effect>>(&mut self, effects: T) {
        self.queue.extend(effects);
    }

    fn next(&mut self) -> Option<Effect> {
        self.queue.pop_front()
    }
}

pub struct Simulator {
    components: Vec<ComponentWrapper>,
    rng: Rng,
    tick_queue: TickQueue,
}

impl Simulator {
    pub fn new(components: Vec<Box<dyn Component>>, rng: Rng) -> Self {
        Self {
            components: components.into_iter().map(ComponentWrapper::new).collect(),
            rng,
            tick_queue: TickQueue::new(),
        }
    }

    fn handle_effects(&mut self, effects: &mut EffectQueue) {
        while let Some(Effect {
            component_index,
            effect,
        }) = effects.next()
        {
            let EffectResult {
                next_tick,
                effects: signals,
            } = effect(&mut self.components[component_index], &mut self.rng);
            self.tick_queue.add_or_update(component_index, next_tick);
            effects.push_all(signals);
        }
    }

    fn tick_without_effects(
        &mut self,
        Tick {
            component_index,
            time,
        }: Tick,
        effects: &mut EffectQueue,
    ) {
        let EffectResult {
            next_tick,
            effects: signals,
        } = self.components[component_index]
            .as_component()
            .tick(time, &mut self.rng);
        self.tick_queue.add_or_update(component_index, next_tick);
        effects.push_all(signals);
    }

    fn first_tick(&mut self) {
        let mut effects = EffectQueue::new();
        for i in 0..self.components.len() {
            self.tick_without_effects(
                Tick {
                    component_index: i,
                    time: 0.,
                },
                &mut effects,
            );
        }
        self.handle_effects(&mut effects);
    }

    fn tick(&mut self, tick: Tick) {
        let mut effects = EffectQueue::new();
        self.tick_without_effects(tick, &mut effects);
        self.handle_effects(&mut effects);
    }

    pub fn run(mut self) {
        self.first_tick();
        while let Some(tick) = self.tick_queue.next() {
            self.tick(tick);
        }
    }
}
