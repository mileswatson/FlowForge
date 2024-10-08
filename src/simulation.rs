use derive_more::From;
use derive_where::derive_where;
use generativity::{Guard, Id};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt::{self, Debug, Formatter},
    rc::Rc,
};
use vec_map::VecMap;

use crate::{quantities::Time, util::logging::Logger};

pub trait HasVariant<P>: From<P> + TryInto<P> {}

impl<E, P> HasVariant<P> for E where E: From<P> + TryInto<P> {}

#[derive(Clone)]
pub struct Clock(Rc<RefCell<Time>>);

impl Clock {
    #[must_use]
    pub fn time(&self) -> Time {
        *self.0.borrow()
    }

    fn set(&self, time: Time) {
        *self.0.borrow_mut() = time;
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct ComponentId<'sim> {
    index: usize,
    sim_id: Id<'sim>,
}

impl<'sim> Debug for ComponentId<'sim> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
pub struct Address<'sim, I, E> {
    create_message: Rc<dyn Fn(I) -> Message<'sim, E> + 'sim>,
}

impl<'sim, I, E> Debug for Address<'sim, I, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Address").finish()
    }
}

impl<'sim, I, E> Address<'sim, I, E> {
    fn new(component_id: ComponentId<'sim>) -> Address<'sim, I, E>
    where
        E: From<I>,
    {
        Address {
            create_message: Rc::new(move |effect| Message {
                destination: component_id,
                effect: effect.into(),
            }),
        }
    }

    #[must_use]
    pub fn cast<J>(self) -> Address<'sim, J, E>
    where
        J: 'sim,
        I: From<J> + 'sim,
        E: 'sim,
    {
        self.manual_cast(I::from)
    }

    pub fn manual_cast<J>(self, f: impl (Fn(J) -> I) + 'sim) -> Address<'sim, J, E>
    where
        I: From<J> + 'sim,
        E: 'sim,
    {
        Address {
            create_message: Rc::new(move |effect| (self.create_message)(f(effect))),
        }
    }

    pub fn create_message(&self, effect: I) -> Message<'sim, E> {
        (self.create_message)(effect)
    }
}

pub struct Message<'sim, E> {
    destination: ComponentId<'sim>,
    effect: E,
}

impl<'sim, E> Message<'sim, E> {
    pub const fn destination(&self) -> ComponentId<'sim> {
        self.destination
    }
}

#[allow(unused_variables)]
pub trait Component<'sim, E>: Debug {
    type Receive;
    fn next_tick(&self, time: Time) -> Option<Time> {
        None
    }
    fn tick(&mut self, time: Time) -> Vec<Message<'sim, E>> {
        panic!()
    }
    fn receive(&mut self, e: Self::Receive, time: Time) -> Vec<Message<'sim, E>> {
        vec![]
    }
}

impl<'sim, E, C> Component<'sim, E> for &mut C
where
    C: Component<'sim, E>,
{
    type Receive = C::Receive;

    fn next_tick(&self, time: Time) -> Option<Time> {
        (**self).next_tick(time)
    }

    fn tick(&mut self, time: Time) -> Vec<Message<'sim, E>> {
        (*self).tick(time)
    }

    fn receive(&mut self, e: Self::Receive, time: Time) -> Vec<Message<'sim, E>> {
        (*self).receive(e, time)
    }
}

impl<'sim, E, C> Component<'sim, E> for Rc<RefCell<C>>
where
    C: Component<'sim, E>,
{
    type Receive = C::Receive;

    fn next_tick(&self, time: Time) -> Option<Time> {
        self.borrow().next_tick(time)
    }

    fn tick(&mut self, time: Time) -> Vec<Message<'sim, E>> {
        self.borrow_mut().tick(time)
    }

    fn receive(&mut self, e: Self::Receive, time: Time) -> Vec<Message<'sim, E>> {
        self.borrow_mut().receive(e, time)
    }
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

    pub fn pop_next(&mut self) -> (Time, Option<usize>) {
        if let Some((idx, time)) = self
            .waiting
            .iter()
            .min_by_key(|&(_, time)| time)
            .map(|(x, t)| (x, *t))
        {
            self.current_time = time;
            self.waiting.remove(idx);
            (time, Some(idx))
        } else {
            (Time::MAX, None)
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

pub struct ComponentSlot<'sim, 'a, 'b, C, E>
where
    C: Component<'sim, E>,
{
    index: usize,
    builder: &'b SimulatorBuilder<'sim, 'a, E>,
    address: Address<'sim, C::Receive, E>,
}

impl<'sim, 'a, 'b, C, E> ComponentSlot<'sim, 'a, 'b, C, E>
where
    C: Component<'sim, E> + 'a,
    E: HasVariant<C::Receive>,
{
    #[must_use]
    pub fn address(&self) -> Address<'sim, C::Receive, E> {
        self.address.clone()
    }

    pub fn fill(self, component: C) -> Address<'sim, C::Receive, E> {
        let mut components = self.builder.components.borrow_mut();
        assert!(components[self.index].is_none());
        components[self.index] = Some(Box::new(AssertWrapper(component)));
        self.address
    }
}

pub struct SimulatorBuilder<'sim, 'a, E> {
    id: Id<'sim>,
    clock: Clock,
    #[allow(clippy::type_complexity)]
    components: RefCell<Vec<Option<Box<dyn Component<'sim, E, Receive = E> + 'a>>>>,
}

#[derive(Debug)]
pub struct EmptySlot;

impl<'sim, 'a, E> SimulatorBuilder<'sim, 'a, E> {
    #[must_use]
    pub fn new(guard: Guard<'sim>) -> SimulatorBuilder<'sim, 'a, E> {
        SimulatorBuilder {
            id: guard.into(),
            components: RefCell::new(Vec::new()),
            clock: Clock(Rc::new(RefCell::new(Time::SIM_START))),
        }
    }

    pub fn insert<C>(&self, component: C) -> Address<'sim, C::Receive, E>
    where
        C: Component<'sim, E> + 'a,
        C::Receive: 'sim,
        E: HasVariant<C::Receive> + 'sim,
    {
        let mut components = self.components.borrow_mut();
        let id = ComponentId::new(components.len(), self.id);
        components.push(Some(Box::new(AssertWrapper(component))));
        Address::new(id)
    }

    pub fn reserve_slot<'b, C>(&'b self) -> ComponentSlot<'sim, 'a, 'b, C, E>
    where
        C: Component<'sim, E>,
        C::Receive: 'sim,
        E: From<C::Receive> + 'sim,
    {
        let mut components = self.components.borrow_mut();
        let index = components.len();
        components.push(None);
        ComponentSlot {
            index,
            builder: self,
            address: Address::new(ComponentId::new(index, self.id)),
        }
    }

    pub fn build<L>(self, logger: L) -> Result<Simulator<'sim, 'a, E, L>, EmptySlot> {
        let components = self
            .components
            .into_inner()
            .into_iter()
            .collect::<Option<Vec<_>>>();
        components
            .map(|components| {
                let mut tick_queue = TickQueue::with_capacity(components.len());
                components.iter().enumerate().for_each(|(idx, component)| {
                    tick_queue.update(idx, component.next_tick(Time::SIM_START));
                });
                let (next_time, next_tick) = tick_queue.pop_next();
                self.clock.set(next_time);
                Simulator {
                    id: self.id,
                    tick_queue,
                    components,
                    logger,
                    clock: self.clock,
                    next_tick,
                }
            })
            .ok_or(EmptySlot)
    }

    pub fn clock(&self) -> Clock {
        self.clock.clone()
    }
}

#[derive(Debug, From)]
struct AssertWrapper<C>(C);

impl<'sim, C, E> Component<'sim, E> for AssertWrapper<C>
where
    C: Component<'sim, E>,
    E: HasVariant<C::Receive>,
{
    type Receive = E;

    fn next_tick(&self, time: Time) -> Option<Time> {
        self.0.next_tick(time)
    }

    fn tick(&mut self, time: Time) -> Vec<Message<'sim, E>> {
        self.0.tick(time)
    }

    fn receive(&mut self, e: E, time: Time) -> Vec<Message<'sim, E>> {
        let e = e
            .try_into()
            .map_or_else(|_| panic!("Incorrect message type!"), |x| x);
        self.0.receive(e, time)
    }
}

pub struct Simulator<'sim, 'a, E, L> {
    id: Id<'sim>,
    components: Vec<Box<dyn Component<'sim, E, Receive = E> + 'a>>,
    tick_queue: TickQueue,
    next_tick: Option<usize>,
    clock: Clock,
    logger: L,
}

impl<'sim, 'a, E, L> Simulator<'sim, 'a, E, L>
where
    L: Logger,
{
    fn handle_messages(&mut self, time: Time, effects: &mut EffectQueue<'sim, E>) {
        while let Some(Message {
            destination: component_id,
            effect,
        }) = effects.pop_next()
        {
            assert_eq!(component_id.sim_id, self.id);
            let component = &mut self.components[component_id.index];
            let messages = component.receive(effect, time);
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
        let messages = component.tick(time);
        let next_tick = component.next_tick(time);
        self.tick_queue.update(component_id.index, next_tick);
        effects.push_all(messages);
    }

    pub fn tick(&mut self) -> bool {
        self.next_tick
            .map(|idx| {
                let time = self.clock.time();
                log!(self.logger, "time = {}", &time);
                let mut effects = EffectQueue::new();
                self.tick_without_messages(ComponentId::new(idx, self.id), time, &mut effects);
                self.handle_messages(time, &mut effects);
                let (next_time, next_tick) = self.tick_queue.pop_next();
                self.clock.set(next_time);
                self.next_tick = next_tick;
            })
            .is_some()
    }

    pub fn time(&self) -> Time {
        self.clock.time()
    }
}
