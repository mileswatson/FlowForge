use rustc_hash::{FxHashMap, FxHasher};
use std::{
    cell::{Ref, RefCell, RefMut},
    cmp::Reverse,
    collections::VecDeque,
    fmt::Debug,
    hash::{BuildHasherDefault, Hash},
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use priority_queue::PriorityQueue;

use crate::{
    logging::Logger,
    quantities::{Time, TimeSpan},
    rand::Rng,
};

#[derive(Debug)]
pub enum DynComponent<'a, E> {
    Owned(Box<dyn Component<E> + 'a>),
    Shared(Rc<RefCell<dyn Component<E> + 'a>>),
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
    sim_id: u64,
}

impl ComponentId {
    #[must_use]
    const fn new(index: usize, sim_id: u64) -> ComponentId {
        ComponentId { index, sim_id }
    }
}

pub trait MaybeHasVariant<T>: Sized + Debug + Sync + 'static {
    fn try_into(self) -> Result<T, Self>;
    fn try_case<F, C, R>(self, ctx: C, f: F) -> Result<R, (Self, C)>
    where
        F: FnOnce(T, C) -> R,
    {
        match self.try_into() {
            Ok(t) => Ok(f(t, ctx)),
            Err(s) => Err((s, ctx)),
        }
    }
}

pub fn try_case<E, T, F, C, R>(f: F) -> impl FnOnce((E, C)) -> Result<R, (E, C)>
where
    E: MaybeHasVariant<T>,
    F: FnOnce(T, C) -> R,
{
    |(e, ctx): (E, C)| match e.try_into() {
        Ok(t) => Ok(f(t, ctx)),
        Err(e) => Err((e, ctx)),
    }
}

impl<T> MaybeHasVariant<T> for T
where
    T: Sized + Debug + Sync + 'static,
{
    fn try_into(self) -> Result<T, Self> {
        Ok(self)
    }
}

pub trait HasVariant<T>: From<T> + MaybeHasVariant<T> {}

impl<E, T> HasVariant<T> for E where E: From<T> + MaybeHasVariant<T> {}

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

#[derive(Debug)]
pub struct EffectContext<'a> {
    pub self_id: ComponentId,
    pub time: Time,
    pub rng: &'a mut Rng,
}

pub trait Component<E>: Debug {
    fn next_tick(&self, time: Time) -> Option<Time>;
    fn tick(&mut self, context: EffectContext) -> Vec<Message<E>>;
    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<E>>;
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

static NUM_SIMULATORS: AtomicU64 = AtomicU64::new(0);

pub struct ComponentSlot<'a, 'b, E> {
    index: usize,
    builder: &'b SimulatorBuilder<'a, E>,
}

impl<'a, 'b, E> ComponentSlot<'a, 'b, E> {
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        ComponentId::new(self.index, self.builder.id)
    }

    #[allow(clippy::must_use_candidate)]
    pub fn set(self, component: DynComponent<'a, E>) -> ComponentId {
        let mut components = self.builder.components.borrow_mut();
        assert!(components[self.index].is_none());
        components[self.index] = Some(component);
        self.id()
    }
}

#[derive(Default)]
pub struct SimulatorBuilder<'a, E> {
    id: u64,
    components: RefCell<Vec<Option<DynComponent<'a, E>>>>,
}

impl<'a, E> SimulatorBuilder<'a, E> {
    #[must_use]
    pub fn new() -> SimulatorBuilder<'a, E> {
        SimulatorBuilder {
            id: NUM_SIMULATORS.fetch_add(1, Ordering::Relaxed),
            components: RefCell::new(Vec::new()),
        }
    }

    pub fn insert(&self, component: DynComponent<'a, E>) -> ComponentId {
        let mut components = self.components.borrow_mut();
        let id = ComponentId::new(components.len(), self.id);
        components.push(Some(component));
        id
    }

    pub fn reserve_slot<'b>(&'b self) -> ComponentSlot<'a, 'b, E> {
        let mut components = self.components.borrow_mut();
        let index = components.len();
        components.push(None);
        ComponentSlot {
            index,
            builder: self,
        }
    }

    pub fn build<L>(self, rng: &'a mut Rng, logger: L) -> Simulator<'a, E, L> {
        let components = self
            .components
            .into_inner()
            .into_iter()
            .map(Option::unwrap)
            .collect();
        Simulator {
            id: self.id,
            components,
            rng,
            tick_queue: EventQueue::new(),
            logger,
        }
    }
}

pub struct Simulator<'a, E, L> {
    id: u64,
    components: Vec<DynComponent<'a, E>>,
    rng: &'a mut Rng,
    tick_queue: EventQueue<ComponentId, ()>,
    logger: L,
}

impl<'a, E, L> Simulator<'a, E, L>
where
    E: Debug,
    L: Logger,
{
    fn handle_messages(&mut self, time: Time, effects: &mut EffectQueue<E>) {
        while let Some(Message {
            component_id,
            effect,
        }) = effects.pop_next()
        {
            assert_eq!(component_id.sim_id, self.id);
            let mut component = self.components[component_id.index].borrow_mut();
            let messages = component.receive(
                effect,
                EffectContext {
                    self_id: component_id,
                    time,
                    rng: self.rng,
                },
            );
            let next_tick = component.next_tick(time);
            self.tick_queue
                .insert_or_update(component_id, (), next_tick);
            effects.push_all(messages);
        }
    }

    fn tick_without_messages(
        &mut self,
        component_id: ComponentId,
        time: Time,
        effects: &mut EffectQueue<E>,
    ) {
        assert_eq!(component_id.sim_id, self.id);
        let mut component = self.components[component_id.index].borrow_mut();
        let messages = component.tick(EffectContext {
            self_id: component_id,
            time,
            rng: self.rng,
        });
        let next_tick = component.next_tick(time);
        self.tick_queue
            .insert_or_update(component_id, (), next_tick);
        effects.push_all(messages);
    }

    fn tick(&mut self, component_id: ComponentId, time: Time) {
        log!(self.logger, "time = {}", &time);
        let mut effects = EffectQueue::new();
        self.tick_without_messages(component_id, time, &mut effects);
        self.handle_messages(time, &mut effects);
    }

    pub fn run_for(mut self, timespan: TimeSpan) {
        let end_time = Time::SIM_START + timespan;
        self.components
            .iter()
            .enumerate()
            .for_each(|(idx, component)| {
                self.tick_queue.insert_or_update(
                    ComponentId::new(idx, self.id),
                    (),
                    component.borrow().next_tick(Time::SIM_START),
                );
            });
        while let Some((time, component_id, ())) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(component_id, time);
        }
    }
}
