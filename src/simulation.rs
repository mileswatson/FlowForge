use rustc_hash::{FxHashMap, FxHasher};
use std::{
    cmp::Reverse,
    collections::VecDeque,
    fmt::{Debug, Display},
    hash::{BuildHasherDefault, Hash},
    ops::{Add, Mul, MulAssign, Sub},
};

use ordered_float::NotNan;
use priority_queue::PriorityQueue;

use crate::{logging::Logger, rand::Rng};

pub type Float = f64;

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct TimeSpan {
    ts: Float,
}

impl TimeSpan {
    #[must_use]
    pub const fn new(ts: Float) -> TimeSpan {
        TimeSpan { ts }
    }
}

impl Add for TimeSpan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TimeSpan::new(self.ts + rhs.ts)
    }
}

impl Mul<TimeSpan> for Float {
    type Output = TimeSpan;

    fn mul(self, rhs: TimeSpan) -> Self::Output {
        TimeSpan::new(self * rhs.ts)
    }
}

impl MulAssign<Float> for TimeSpan {
    fn mul_assign(&mut self, rhs: Float) {
        self.ts *= rhs;
    }
}

impl Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.ts)
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct Rate {
    r: Float,
}

impl Rate {
    #[must_use]
    pub const fn new(r: Float) -> Rate {
        Rate { r }
    }

    #[must_use]
    pub fn period(&self) -> TimeSpan {
        TimeSpan::new(1. / self.r)
    }
}

impl Display for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s^-1", self.r)
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct Time {
    t: Float,
}

impl Time {
    pub const MIN: Time = Time { t: Float::MIN };

    const fn from_sim_start(t: Float) -> Time {
        Time { t }
    }

    #[must_use]
    pub const fn sim_start() -> Time {
        Time::from_sim_start(0.)
    }
}

impl Sub<Time> for Time {
    type Output = TimeSpan;

    fn sub(self, Time { t }: Time) -> Self::Output {
        TimeSpan::new(self.t - t)
    }
}

impl Add<TimeSpan> for Time {
    type Output = Time;

    fn add(self, TimeSpan { ts }: TimeSpan) -> Self::Output {
        Time::from_sim_start(self.t + ts)
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}t", self.t)
    }
}

#[must_use]
pub fn earliest(times: &[Option<Time>]) -> Option<Time> {
    times
        .iter()
        .fold(None, |prev, current| match (prev, *current) {
            (Some(Time { t: t1 }), Some(Time { t: t2 })) => {
                Some(Time::from_sim_start(Float::min(t1, t2)))
            }
            (m, None) | (None, m) => m,
        })
}

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

pub struct EffectContext<'a> {
    pub self_index: usize,
    pub time: Time,
    pub rng: &'a mut Rng,
}

pub trait Component<E> {
    fn tick(&mut self, context: EffectContext) -> EffectResult<E>;
    fn receive(&mut self, e: E, context: EffectContext) -> EffectResult<E>;
}

#[derive(Debug)]
pub struct EventQueue<I: Hash + Eq, E> {
    current_time: Time,
    waiting: FxHashMap<I, E>,
    queue: PriorityQueue<I, Reverse<NotNan<Float>>, BuildHasherDefault<FxHasher>>,
}

impl<I: Hash + Eq + Copy, E> EventQueue<I, E> {
    #[must_use]
    pub fn new() -> EventQueue<I, E> {
        EventQueue {
            current_time: Time::MIN,
            waiting: FxHashMap::default(),
            queue: PriorityQueue::<_, _, BuildHasherDefault<FxHasher>>::with_default_hasher(),
        }
    }

    pub fn update(&mut self, id: I, time: Option<Time>) {
        if let Some(time) = time {
            assert!(time >= self.current_time);
            self.queue.push(id, Reverse(NotNan::new(time.t).unwrap()));
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
        self.queue
            .peek()
            .map(|(_, Reverse(x))| Time::from_sim_start(**x))
    }

    pub fn pop_next(&mut self) -> Option<(Time, I, E)> {
        if let Some((component_index, Reverse(time))) = self.queue.pop() {
            self.current_time = Time::from_sim_start(*time);
            Some((
                Time::from_sim_start(*time),
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
    components: Vec<Box<dyn Component<E> + 'a>>,
    rng: Rng,
    tick_queue: EventQueue<usize, ()>,
    logger: L,
}

impl<'a, E, L> Simulator<'a, E, L>
where
    L: Logger,
{
    #[must_use]
    pub fn new(
        components: Vec<Box<dyn Component<E> + 'a>>,
        rng: Rng,
        logger: L,
    ) -> Simulator<'a, E, L> {
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
            } = self.components[component_index].receive(
                effect,
                EffectContext {
                    self_index: component_index,
                    time,
                    rng: &mut self.rng,
                },
            );
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
        } = self.components[component_index].tick(EffectContext {
            self_index: component_index,
            time,
            rng: &mut self.rng,
        });
        self.tick_queue
            .insert_or_update(component_index, (), next_tick);
        effects.push_all(signals);
    }

    fn first_tick(&mut self) {
        log!(self.logger, "time = 0.0");
        let sim_start = Time::sim_start();
        let mut effects = EffectQueue::new();
        for i in 0..self.components.len() {
            self.tick_without_effects(i, sim_start, &mut effects);
        }
        self.handle_effects(sim_start, &mut effects);
    }

    fn tick(&mut self, component_index: usize, time: Time) {
        log!(self.logger, "time = {}", &time);
        let mut effects = EffectQueue::new();
        self.tick_without_effects(component_index, time, &mut effects);
        self.handle_effects(time, &mut effects);
    }

    pub fn run_for(mut self, timespan: TimeSpan) {
        let end_time = Time::sim_start() + timespan;
        self.first_tick();
        while let Some((time, component_index, ())) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(component_index, time);
        }
    }
}
