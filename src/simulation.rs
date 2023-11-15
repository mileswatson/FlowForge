use rustc_hash::{FxHashMap, FxHasher};
use std::{
    cell::{Ref, RefCell, RefMut},
    cmp::Reverse,
    collections::VecDeque,
    fmt::Debug,
    hash::{BuildHasherDefault, Hash},
    ops::{Deref, DerefMut},
    rc::Rc,
};

use priority_queue::PriorityQueue;

use crate::{
    logging::Logger,
    rand::Rng,
    time::{Time, TimeSpan},
};

pub enum DynComponent<'a, E> {
    Owned(Box<dyn Component<E>>),
    Shared(Rc<RefCell<dyn Component<E>>>),
    Ref(&'a mut (dyn Component<E> + 'a)),
}

impl<'a, E> DynComponent<'a, E> {
    #[must_use]
    pub fn new<T: Component<E> + 'static>(value: T) -> DynComponent<'a, E> {
        DynComponent::Owned(Box::new(value))
    }

    #[must_use]
    pub fn owned(value: Box<dyn Component<E>>) -> DynComponent<'a, E> {
        DynComponent::Owned(value)
    }

    #[must_use]
    pub fn shared(value: Rc<RefCell<dyn Component<E>>>) -> DynComponent<'a, E> {
        DynComponent::Shared(value)
    }

    #[must_use]
    pub fn reference(value: &'a mut dyn Component<E>) -> DynComponent<'a, E> {
        DynComponent::Ref(value)
    }
}

pub enum DynComponentRef<'a, E> {
    Ref(&'a dyn Component<E>),
    ScopedRef(Ref<'a, dyn Component<E>>),
}

pub enum DynComponentRefMut<'a, E> {
    Ref(&'a mut (dyn Component<E>)),
    ScopedRef(RefMut<'a, dyn Component<E>>),
}

impl<'a, E> DynComponent<'a, E> {
    #[must_use]
    pub fn borrow(&self) -> DynComponentRef<E> {
        match self {
            DynComponent::Owned(x) => DynComponentRef::Ref(x.as_ref()),
            DynComponent::Shared(x) => DynComponentRef::ScopedRef(x.borrow()),
            DynComponent::Ref(r) => DynComponentRef::Ref(*r),
        }
    }

    #[must_use]
    pub fn borrow_mut(&mut self) -> DynComponentRefMut<E> {
        match self {
            DynComponent::Owned(x) => DynComponentRefMut::Ref(x.as_mut()),
            DynComponent::Shared(x) => DynComponentRefMut::ScopedRef(x.borrow_mut()),
            DynComponent::Ref(r) => DynComponentRefMut::Ref(*r),
        }
    }
}

impl<'a, E> Deref for DynComponentRef<'a, E> {
    type Target = dyn Component<E> + 'a;

    fn deref(&self) -> &(dyn Component<E> + 'a) {
        match self {
            DynComponentRef::Ref(r) => *r,
            DynComponentRef::ScopedRef(s) => &**s,
        }
    }
}

impl<'a, E> Deref for DynComponentRefMut<'a, E> {
    type Target = dyn Component<E> + 'a;

    fn deref(&self) -> &(dyn Component<E> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &**s,
        }
    }
}

impl<'a, E> DerefMut for DynComponentRefMut<'a, E> {
    fn deref_mut(&mut self) -> &mut (dyn Component<E> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &mut **s,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct ComponentId {
    index: usize,
}

impl ComponentId {
    #[must_use]
    pub const fn new(index: usize) -> ComponentId {
        ComponentId { index }
    }
}

pub trait HasVariant<T>: From<T> + Debug + Sync + 'static {
    fn try_into(self) -> Result<T, Self>;
}

impl<E> HasVariant<E> for E
where
    E: Debug + Sync + 'static,
{
    fn try_into(self) -> Result<E, Self> {
        Ok(self)
    }
}

pub struct Message<E> {
    pub component_id: ComponentId,
    pub effect: E,
}

impl<E> Message<E> {
    pub fn new<V>(component_id: ComponentId, effect: V) -> Message<E>
    where
        E: From<V>,
    {
        Message {
            component_id,
            effect: E::from(effect),
        }
    }
}

pub struct EffectResult<E> {
    pub next_tick: Option<Time>,
    pub effects: Vec<Message<E>>,
}

pub struct EffectContext<'a> {
    pub self_id: ComponentId,
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
    queue: PriorityQueue<I, Reverse<Time>, BuildHasherDefault<FxHasher>>,
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
            self.queue.push(id, Reverse(time));
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
        self.queue.peek().map(|(_, Reverse(x))| *x)
    }

    pub fn pop_next(&mut self) -> Option<(Time, I, E)> {
        if let Some((component_id, Reverse(time))) = self.queue.pop() {
            self.current_time = time;
            Some((
                time,
                component_id,
                self.waiting.remove(&component_id).unwrap(),
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
    components: Vec<DynComponent<'a, E>>,
    rng: &'a mut Rng,
    tick_queue: EventQueue<ComponentId, ()>,
    logger: L,
}

impl<'a, E, L> Simulator<'a, E, L>
where
    L: Logger,
{
    #[must_use]
    pub fn new(
        components: Vec<DynComponent<'a, E>>,
        rng: &'a mut Rng,
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
            component_id,
            effect,
        }) = effects.pop_next()
        {
            let EffectResult {
                next_tick,
                effects: signals,
            } = self.components[component_id.index].borrow_mut().receive(
                effect,
                EffectContext {
                    self_id: component_id,
                    time,
                    rng: self.rng,
                },
            );
            self.tick_queue
                .insert_or_update(component_id, (), next_tick);
            effects.push_all(signals);
        }
    }

    fn tick_without_effects(
        &mut self,
        component_id: ComponentId,
        time: Time,
        effects: &mut EffectQueue<E>,
    ) {
        let EffectResult {
            next_tick,
            effects: signals,
        } = self.components[component_id.index]
            .borrow_mut()
            .tick(EffectContext {
                self_id: component_id,
                time,
                rng: self.rng,
            });
        self.tick_queue
            .insert_or_update(component_id, (), next_tick);
        effects.push_all(signals);
    }

    fn first_tick(&mut self) {
        log!(self.logger, "time = 0.0");
        let sim_start = Time::sim_start();
        let mut effects = EffectQueue::new();
        for i in 0..self.components.len() {
            self.tick_without_effects(ComponentId::new(i), sim_start, &mut effects);
        }
        self.handle_effects(sim_start, &mut effects);
    }

    fn tick(&mut self, component_id: ComponentId, time: Time) {
        log!(self.logger, "time = {}", &time);
        let mut effects = EffectQueue::new();
        self.tick_without_effects(component_id, time, &mut effects);
        self.handle_effects(time, &mut effects);
    }

    pub fn run_for(mut self, timespan: TimeSpan) {
        let end_time = Time::sim_start() + timespan;
        self.first_tick();
        while let Some((time, component_id, ())) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(component_id, time);
        }
    }
}
