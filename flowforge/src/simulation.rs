use generativity::{Guard, Id};
use itertools::Itertools;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::VecDeque,
    fmt::Debug,
    ops::{Deref, DerefMut},
    rc::Rc,
};
use vec_map::VecMap;

use crate::{
    logging::Logger,
    quantities::{Time, TimeSpan},
};

#[derive(Debug)]
pub enum DynComponent<'sim, 'a, E> {
    Owned(Box<dyn Component<'sim, E> + 'a>),
    Shared(Rc<RefCell<dyn Component<'sim, E> + 'a>>),
    Ref(&'a mut (dyn Component<'sim, E> + 'a)),
}

impl<'sim, 'a, E> DynComponent<'sim, 'a, E> {
    #[must_use]
    pub fn new<T: Component<'sim, E> + 'sim>(value: T) -> DynComponent<'sim, 'a, E> {
        DynComponent::Owned(Box::new(value))
    }

    #[must_use]
    pub fn owned(value: Box<dyn Component<'sim, E>>) -> DynComponent<'sim, 'a, E> {
        DynComponent::Owned(value)
    }

    #[must_use]
    pub fn shared(value: Rc<RefCell<dyn Component<'sim, E> + 'sim>>) -> DynComponent<'sim, 'a, E> {
        DynComponent::Shared(value)
    }

    #[must_use]
    pub fn reference(value: &'a mut dyn Component<'sim, E>) -> DynComponent<'sim, 'a, E> {
        DynComponent::Ref(value)
    }
}

pub enum DynComponentRef<'sim, 'a, E> {
    Ref(&'a dyn Component<'sim, E>),
    ScopedRef(Ref<'a, dyn Component<'sim, E>>),
}

pub enum DynComponentRefMut<'sim, 'a, E> {
    Ref(&'a mut (dyn Component<'sim, E>)),
    ScopedRef(RefMut<'a, dyn Component<'sim, E>>),
}

impl<'sim, 'a, E> DynComponent<'sim, 'a, E> {
    #[must_use]
    pub fn borrow(&self) -> DynComponentRef<'sim, '_, E> {
        match self {
            DynComponent::Owned(x) => DynComponentRef::Ref(x.as_ref()),
            DynComponent::Shared(x) => DynComponentRef::ScopedRef(x.borrow()),
            DynComponent::Ref(r) => DynComponentRef::Ref(*r),
        }
    }

    #[must_use]
    pub fn borrow_mut(&mut self) -> DynComponentRefMut<'sim, '_, E> {
        match self {
            DynComponent::Owned(x) => DynComponentRefMut::Ref(x.as_mut()),
            DynComponent::Shared(x) => DynComponentRefMut::ScopedRef(x.borrow_mut()),
            DynComponent::Ref(r) => DynComponentRefMut::Ref(*r),
        }
    }
}

impl<'sim, 'a, E> Deref for DynComponentRef<'sim, 'a, E> {
    type Target = dyn Component<'sim, E> + 'a;

    fn deref(&self) -> &(dyn Component<'sim, E> + 'a) {
        match self {
            DynComponentRef::Ref(r) => *r,
            DynComponentRef::ScopedRef(s) => &**s,
        }
    }
}

impl<'sim, 'a, E> Deref for DynComponentRefMut<'sim, 'a, E> {
    type Target = dyn Component<'sim, E> + 'a;

    fn deref(&self) -> &(dyn Component<'sim, E> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &**s,
        }
    }
}

impl<'sim, 'a, E> DerefMut for DynComponentRefMut<'sim, 'a, E> {
    fn deref_mut(&mut self) -> &mut (dyn Component<'sim, E> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &mut **s,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct ComponentId<'sim> {
    index: usize,
    sim_id: Id<'sim>,
}

impl<'sim> ComponentId<'sim> {
    #[must_use]
    const fn new(index: usize, sim_id: Id<'sim>) -> ComponentId {
        ComponentId { index, sim_id }
    }
}

pub trait MaybeHasVariant<'a, T>: Sized + Debug + Sync + 'a {
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

pub fn try_case<'a, E, T, F, C, R>(f: F) -> impl FnOnce((E, C)) -> Result<R, (E, C)>
where
    E: MaybeHasVariant<'a, T>,
    F: FnOnce(T, C) -> R,
{
    |(e, ctx): (E, C)| match e.try_into() {
        Ok(t) => Ok(f(t, ctx)),
        Err(e) => Err((e, ctx)),
    }
}

impl<'a, T> MaybeHasVariant<'a, T> for T
where
    T: Sized + Debug + Sync + 'static,
{
    fn try_into(self) -> Result<T, Self> {
        Ok(self)
    }
}

pub trait HasVariant<'a, T>: From<T> + MaybeHasVariant<'a, T> {}

impl<'a, E, T> HasVariant<'a, T> for E where E: From<T> + MaybeHasVariant<'a, T> {}

pub struct Message<'sim, E> {
    pub component_id: ComponentId<'sim>,
    pub effect: E,
}

impl<E> Message<'_, E> {
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
pub struct EffectContext<'sim> {
    pub self_id: ComponentId<'sim>,
    pub time: Time,
}

pub trait Component<'sim, E>: Debug {
    fn next_tick(&self, time: Time) -> Option<Time>;
    fn tick(&mut self, context: EffectContext<'sim>) -> Vec<Message<'sim, E>>;
    fn receive(&mut self, e: E, context: EffectContext<'sim>) -> Vec<Message<'sim, E>>;
}

#[derive(Debug)]
/// TODO: Heapify?
pub struct TickQueue {
    current_time: Time,
    waiting: VecMap<Time>,
}

impl TickQueue {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> TickQueue {
        TickQueue {
            current_time: Time::MIN,
            waiting: VecMap::with_capacity(capacity),
        }
    }

    pub fn update(&mut self, id: usize, time: Option<Time>) {
        if let Some(time) = time {
            assert!(time >= self.current_time);
            self.waiting.insert(id, time);
        } else {
            self.waiting.remove(id);
        }
    }

    #[must_use]
    pub fn next_time(&self) -> Option<Time> {
        self.waiting.values().min().copied()
    }

    pub fn pop_next(&mut self) -> Option<(Time, usize)> {
        if let Some((idx, time)) = self
            .waiting
            .iter()
            .min_by_key(|&(_, time)| time)
            .map(|(x, t)| (x, *t))
        {
            self.current_time = time;
            self.waiting.remove(idx);
            Some((time, idx))
        } else {
            None
        }
    }
}

struct EffectQueue<'sim, E> {
    queue: VecDeque<Message<'sim, E>>,
}

impl<'sim, E> EffectQueue<'sim, E> {
    const fn new() -> EffectQueue<'sim, E> {
        EffectQueue {
            queue: VecDeque::new(),
        }
    }

    fn push_all<T: IntoIterator<Item = Message<'sim, E>>>(&mut self, effects: T) {
        self.queue.extend(effects);
    }

    fn pop_next(&mut self) -> Option<Message<'sim, E>> {
        self.queue.pop_front()
    }
}

pub struct ComponentSlot<'sim, 'a, 'b, E> {
    index: usize,
    builder: &'b SimulatorBuilder<'sim, 'a, E>,
}

impl<'sim, 'a, 'b, E> ComponentSlot<'sim, 'a, 'b, E> {
    #[must_use]
    pub const fn id(&self) -> ComponentId<'sim> {
        ComponentId::new(self.index, self.builder.id)
    }

    #[allow(clippy::must_use_candidate)]
    pub fn set(self, component: DynComponent<'sim, 'a, E>) -> ComponentId<'sim> {
        let mut components = self.builder.components.borrow_mut();
        assert!(components[self.index].is_none());
        components[self.index] = Some(component);
        self.id()
    }
}

pub struct SimulatorBuilder<'sim, 'a, E> {
    id: Id<'sim>,
    components: RefCell<Vec<Option<DynComponent<'sim, 'a, E>>>>,
}

impl<'sim, 'a, E> SimulatorBuilder<'sim, 'a, E> {
    #[must_use]
    pub fn new(guard: Guard<'sim>) -> SimulatorBuilder<'sim, 'a, E> {
        SimulatorBuilder {
            id: guard.into(),
            components: RefCell::new(Vec::new()),
        }
    }

    pub fn insert(&self, component: DynComponent<'sim, 'a, E>) -> ComponentId<'sim> {
        let mut components = self.components.borrow_mut();
        let id = ComponentId::new(components.len(), self.id);
        components.push(Some(component));
        id
    }

    pub fn reserve_slot<'b>(&'b self) -> ComponentSlot<'sim, 'a, 'b, E> {
        let mut components = self.components.borrow_mut();
        let index = components.len();
        components.push(None);
        ComponentSlot {
            index,
            builder: self,
        }
    }

    pub fn build<L>(self, logger: L) -> Simulator<'sim, 'a, E, L> {
        let components = self
            .components
            .into_inner()
            .into_iter()
            .map(Option::unwrap)
            .collect_vec();
        Simulator {
            id: self.id,
            tick_queue: TickQueue::with_capacity(components.len()),
            components,
            logger,
        }
    }
}

pub struct Simulator<'sim, 'a, E, L> {
    id: Id<'sim>,
    components: Vec<DynComponent<'sim, 'a, E>>,
    tick_queue: TickQueue,
    logger: L,
}

impl<'sim, 'a, E, L> Simulator<'sim, 'a, E, L>
where
    E: Debug,
    L: Logger,
{
    fn handle_messages(&mut self, time: Time, effects: &mut EffectQueue<'sim, E>) {
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
                },
            );
            let next_tick = component.next_tick(time);
            self.tick_queue.update(component_id.index, next_tick);
            effects.push_all(messages);
        }
    }

    fn tick_without_messages(
        &mut self,
        component_id: ComponentId<'sim>,
        time: Time,
        effects: &mut EffectQueue<'sim, E>,
    ) {
        let mut component = self.components[component_id.index].borrow_mut();
        let messages = component.tick(EffectContext {
            self_id: component_id,
            time,
        });
        let next_tick = component.next_tick(time);
        self.tick_queue.update(component_id.index, next_tick);
        effects.push_all(messages);
    }

    fn tick(&mut self, component_id: ComponentId<'sim>, time: Time) {
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
                self.tick_queue
                    .update(idx, component.borrow().next_tick(Time::SIM_START));
            });
        while let Some((time, idx)) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(ComponentId::new(idx, self.id), time);
        }
    }
}