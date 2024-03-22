use derive_where::derive_where;
use generativity::{Guard, Id};
use itertools::Itertools;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::VecDeque,
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};
use vec_map::VecMap;

use crate::{
    logging::Logger,
    quantities::{Time, TimeSpan},
};

pub trait HasSubEffect<P>: From<P> + TryInto<P> {}

impl<E, P> HasSubEffect<P> for E where E: From<P> + TryInto<P> {}

#[derive_where(Debug)]
pub enum DynComponent<'sim, 'a, P, E> {
    Owned(Box<dyn Component<'sim, E, Receive = P> + 'a>),
    Shared(Rc<RefCell<dyn Component<'sim, E, Receive = P> + 'a>>),
    Ref(&'a mut (dyn Component<'sim, E, Receive = P> + 'a)),
}

impl<'sim, 'a, P, E> DynComponent<'sim, 'a, P, E> {
    #[must_use]
    pub fn new<T: Component<'sim, E, Receive = P> + 'a>(value: T) -> DynComponent<'sim, 'a, P, E> {
        DynComponent::Owned(Box::new(value))
    }

    #[must_use]
    pub fn owned(
        value: Box<dyn Component<'sim, E, Receive = P> + 'a>,
    ) -> DynComponent<'sim, 'a, P, E> {
        DynComponent::Owned(value)
    }

    #[must_use]
    pub fn shared(
        value: Rc<RefCell<dyn Component<'sim, E, Receive = P> + 'a>>,
    ) -> DynComponent<'sim, 'a, P, E> {
        DynComponent::Shared(value)
    }

    #[must_use]
    pub fn reference(
        value: &'a mut (dyn Component<'sim, E, Receive = P> + 'a),
    ) -> DynComponent<'sim, 'a, P, E> {
        DynComponent::Ref(value)
    }
}

pub enum DynComponentRef<'sim, 'a, P, E> {
    Ref(&'a dyn Component<'sim, E, Receive = P>),
    ScopedRef(Ref<'a, dyn Component<'sim, E, Receive = P>>),
}

pub enum DynComponentRefMut<'sim, 'a, P, E> {
    Ref(&'a mut (dyn Component<'sim, E, Receive = P>)),
    ScopedRef(RefMut<'a, dyn Component<'sim, E, Receive = P>>),
}

impl<'sim, 'a, P, E> DynComponent<'sim, 'a, P, E> {
    #[must_use]
    pub fn borrow(&self) -> DynComponentRef<'sim, '_, P, E> {
        match self {
            DynComponent::Owned(x) => DynComponentRef::Ref(x.as_ref()),
            DynComponent::Shared(x) => DynComponentRef::ScopedRef(x.borrow()),
            DynComponent::Ref(r) => DynComponentRef::Ref(*r),
        }
    }

    #[must_use]
    pub fn borrow_mut(&mut self) -> DynComponentRefMut<'sim, '_, P, E> {
        match self {
            DynComponent::Owned(x) => DynComponentRefMut::Ref(x.as_mut()),
            DynComponent::Shared(x) => DynComponentRefMut::ScopedRef(x.borrow_mut()),
            DynComponent::Ref(r) => DynComponentRefMut::Ref(*r),
        }
    }
}

impl<'sim, 'a, P, E> Deref for DynComponentRef<'sim, 'a, P, E> {
    type Target = dyn Component<'sim, E, Receive = P> + 'a;

    fn deref(&self) -> &(dyn Component<'sim, E, Receive = P> + 'a) {
        match self {
            DynComponentRef::Ref(r) => *r,
            DynComponentRef::ScopedRef(s) => &**s,
        }
    }
}

impl<'sim, 'a, P, E> Deref for DynComponentRefMut<'sim, 'a, P, E> {
    type Target = dyn Component<'sim, E, Receive = P> + 'a;

    fn deref(&self) -> &(dyn Component<'sim, E, Receive = P> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &**s,
        }
    }
}

impl<'sim, 'a, P, E> DerefMut for DynComponentRefMut<'sim, 'a, P, E> {
    fn deref_mut(&mut self) -> &mut (dyn Component<'sim, E, Receive = P> + 'a) {
        match self {
            DynComponentRefMut::Ref(r) => *r,
            DynComponentRefMut::ScopedRef(s) => &mut **s,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct ComponentId<'sim> {
    index: usize,
    sim_id: Id<'sim>,
}

impl<'sim> Debug for ComponentId<'sim> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ComponentId").field(&self.index).finish()
    }
}

impl<'sim> ComponentId<'sim> {
    #[must_use]
    const fn new(index: usize, sim_id: Id<'sim>) -> ComponentId {
        ComponentId { index, sim_id }
    }
}

#[derive_where(Clone)]
pub struct MessageDestination<'sim, I, E> {
    create_message: Rc<dyn Fn(I) -> Message<'sim, E> + 'sim>,
}

impl<'sim, I, E> Debug for MessageDestination<'sim, I, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageDestination").finish()
    }
}

impl<'sim, T> MessageDestination<'sim, T, T> {
    fn new(component_id: ComponentId<'sim>) -> MessageDestination<'sim, T, T> {
        MessageDestination {
            create_message: Rc::new(move |effect| Message {
                component_id,
                effect,
            }),
        }
    }
}

impl<'sim, I, E> MessageDestination<'sim, I, E> {
    #[must_use]
    pub fn cast<J>(self) -> MessageDestination<'sim, J, E>
    where
        I: From<J> + 'sim,
        E: 'sim,
    {
        MessageDestination {
            create_message: Rc::new(move |effect| (self.create_message)(effect.into())),
        }
    }

    pub fn create_message(&self, effect: I) -> Message<'sim, E> {
        (self.create_message)(effect)
    }
}

pub struct Message<'sim, E> {
    component_id: ComponentId<'sim>,
    effect: E,
}

impl<'sim, E> Message<'sim, E> {
    pub const fn destination(&self) -> ComponentId<'sim> {
        self.component_id
    }
}

#[derive(Debug)]
pub struct EffectContext {
    pub time: Time,
}

pub trait Component<'sim, E>: Debug {
    type Receive;
    fn next_tick(&self, time: Time) -> Option<Time>;
    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>>;
    fn receive(&mut self, e: Self::Receive, context: EffectContext) -> Vec<Message<'sim, E>>;
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

pub struct ComponentSlot<'sim, 'a, 'b, P, E> {
    index: usize,
    builder: &'b SimulatorBuilder<'sim, 'a, E>,
    receive: PhantomData<P>,
    destination: MessageDestination<'sim, P, E>,
}

impl<'sim, 'a, 'b, P, E> ComponentSlot<'sim, 'a, 'b, P, E>
where
    E: HasSubEffect<P>,
{
    #[must_use]
    pub fn destination(&self) -> MessageDestination<'sim, P, E>
    where
        E: HasSubEffect<P>,
    {
        self.destination.clone()
    }

    #[allow(clippy::must_use_candidate)]
    pub fn set(self, component: DynComponent<'sim, 'a, P, E>) -> MessageDestination<'sim, P, E> {
        let mut components = self.builder.components.borrow_mut();
        assert!(components[self.index].is_none());
        components[self.index] = Some(Box::new(ComponentWrapper::new(component)));
        self.destination
    }
}

pub struct SimulatorBuilder<'sim, 'a, E> {
    id: Id<'sim>,
    #[allow(clippy::type_complexity)]
    components: RefCell<Vec<Option<Box<dyn Component<'sim, E, Receive = E> + 'a>>>>,
}

impl<'sim, 'a, E> SimulatorBuilder<'sim, 'a, E> {
    #[must_use]
    pub fn new(guard: Guard<'sim>) -> SimulatorBuilder<'sim, 'a, E> {
        SimulatorBuilder {
            id: guard.into(),
            components: RefCell::new(Vec::new()),
        }
    }

    pub fn insert<P>(
        &self,
        component: DynComponent<'sim, 'a, P, E>,
    ) -> MessageDestination<'sim, P, E>
    where
        E: HasSubEffect<P> + 'sim,
    {
        let mut components = self.components.borrow_mut();
        let id = ComponentId::new(components.len(), self.id);
        components.push(Some(Box::new(ComponentWrapper::new(component))));
        MessageDestination::new(id).cast()
    }

    pub fn reserve_slot<'b, P>(&'b self) -> ComponentSlot<'sim, 'a, 'b, P, E>
    where
        E: From<P> + 'sim,
    {
        let mut components = self.components.borrow_mut();
        let index = components.len();
        components.push(None);
        ComponentSlot {
            index,
            builder: self,
            receive: PhantomData,
            destination: MessageDestination::new(ComponentId::new(index, self.id)).cast(),
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

pub trait GenericComponent<'sim, 'a, E> {
    fn next_tick(&self, time: Time) -> Option<Time>;
    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>>;
    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<'sim, E>>;
}

#[derive_where(Debug)]
struct ComponentWrapper<'sim, 'a, P, E> {
    inner: DynComponent<'sim, 'a, P, E>,
}

impl<'sim, 'a, P, E> ComponentWrapper<'sim, 'a, P, E> {
    pub const fn new(inner: DynComponent<'sim, 'a, P, E>) -> ComponentWrapper<'sim, 'a, P, E> {
        ComponentWrapper { inner }
    }
}

impl<'sim, 'a, P, E> Component<'sim, E> for ComponentWrapper<'sim, 'a, P, E>
where
    E: HasSubEffect<P>,
{
    type Receive = E;

    fn next_tick(&self, time: Time) -> Option<Time> {
        self.inner.borrow().next_tick(time)
    }

    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>> {
        self.inner.borrow_mut().tick(context)
    }

    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<'sim, E>> {
        self.inner.borrow_mut().receive(
            e.try_into()
                .map_or_else(|_| panic!("Incorrect message type!"), |x| x),
            context,
        )
    }
}

pub struct Simulator<'sim, 'a, E, L> {
    id: Id<'sim>,
    components: Vec<Box<dyn Component<'sim, E, Receive = E> + 'a>>,
    tick_queue: TickQueue,
    logger: L,
}

impl<'sim, 'a, E, L> Simulator<'sim, 'a, E, L>
where
    L: Logger,
{
    fn handle_messages(&mut self, time: Time, effects: &mut EffectQueue<'sim, E>) {
        while let Some(Message {
            component_id,
            effect,
        }) = effects.pop_next()
        {
            assert_eq!(component_id.sim_id, self.id);
            let component = &mut self.components[component_id.index];
            let messages = component.receive(effect, EffectContext { time });
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
        let component = &mut self.components[component_id.index];
        let messages = component.tick(EffectContext { time });
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
                    .update(idx, component.next_tick(Time::SIM_START));
            });
        while let Some((time, idx)) = self.tick_queue.pop_next() {
            if time >= end_time {
                break;
            }
            self.tick(ComponentId::new(idx, self.id), time);
        }
    }
}
