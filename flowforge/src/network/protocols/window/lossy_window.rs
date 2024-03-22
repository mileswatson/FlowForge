use std::{cell::RefCell, fmt::Debug, marker::PhantomData, rc::Rc};

use derive_more::{From, TryInto};
use derive_where::derive_where;
use itertools::Itertools;

use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties, NoPacketsAcked},
    logging::Logger,
    meters::{DisabledInfoRateMeter, EnabledInfoRateMeter, InfoRateMeterNeverEnabled, Mean},
    network::{toggler::Toggle, Packet, PacketDestination},
    quantities::{latest, InformationRate, Time, TimeSpan},
    simulation::{
        Component, ComponentSlot, DynComponent, EffectContext, HasSubEffect, Message,
        MessageDestination, SimulatorBuilder,
    },
};

#[derive(Debug, Clone)]
pub struct LossyWindowSettings {
    pub window: u32,
    pub intersend_delay: TimeSpan,
}

pub trait LossyWindowBehavior: Debug {
    fn initial_settings(&self) -> LossyWindowSettings;
    fn ack_received<L: Logger>(
        &mut self,
        context: AckReceived,
        logger: &mut L,
    ) -> Option<LossyWindowSettings>;
}

pub struct AckReceived {
    pub current_settings: LossyWindowSettings,
    pub sent_time: Time,
    pub received_time: Time,
}

#[derive(From, TryInto)]
pub enum LossyWindowControllerEffect {
    Toggle(Toggle),
    AckReceived(AckReceived),
}

#[derive(Debug)]
enum LossyWindowControllerState<B> {
    Enabled(B),
    Disabled { wait_for_enable: bool },
}

pub struct LossyWindowController<'sim, 'a, B, E, L> {
    sender: MessageDestination<'sim, LossySenderEffect<'sim, E>, E>,
    new_behavior: Box<dyn (Fn() -> B) + 'a>,
    state: LossyWindowControllerState<B>,
    logger: L,
}

impl<'sim, 'a, B: Debug, E, L: Debug> Debug for LossyWindowController<'sim, 'a, B, E, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossyWindowController")
            .field("sender", &self.sender)
            .field("state", &self.state)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<'sim, 'a, B, E, L> LossyWindowController<'sim, 'a, B, E, L> {
    pub fn new(
        sender: MessageDestination<'sim, LossySenderEffect<'sim, E>, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> LossyWindowController<'sim, 'a, B, E, L> {
        LossyWindowController {
            sender,
            new_behavior,
            state: LossyWindowControllerState::Disabled { wait_for_enable },
            logger,
        }
    }
}

impl<'sim, 'a, B, E, L> Component<'sim, E> for LossyWindowController<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior,
    L: Logger,
{
    type Receive = LossyWindowControllerEffect;

    fn next_tick(&self, time: Time) -> Option<Time> {
        if matches!(
            self.state,
            LossyWindowControllerState::Disabled {
                wait_for_enable: false,
            }
        ) {
            Some(time)
        } else {
            None
        }
    }

    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>> {
        if matches!(
            self.state,
            LossyWindowControllerState::Disabled {
                wait_for_enable: false,
            }
        ) {
            self.receive(LossyWindowControllerEffect::Toggle(Toggle::Enable), context)
        } else {
            panic!()
        }
    }

    fn receive(&mut self, e: Self::Receive, _context: EffectContext) -> Vec<Message<'sim, E>> {
        (match (&mut self.state, e) {
            (
                LossyWindowControllerState::Disabled { .. },
                LossyWindowControllerEffect::Toggle(Toggle::Enable),
            ) => {
                let behavior = (self.new_behavior)();
                let initial_settings = behavior.initial_settings();
                self.state = LossyWindowControllerState::Enabled(behavior);
                Some(SettingsUpdate::Enable(initial_settings))
            }
            (
                LossyWindowControllerState::Enabled(_),
                LossyWindowControllerEffect::Toggle(Toggle::Disable),
            ) => {
                self.state = LossyWindowControllerState::Disabled {
                    wait_for_enable: true,
                };
                Some(SettingsUpdate::Disable)
            }
            (
                LossyWindowControllerState::Enabled(behavior),
                LossyWindowControllerEffect::AckReceived(context),
            ) => behavior
                .ack_received(context, &mut self.logger)
                .map(SettingsUpdate::Enable),
            (
                LossyWindowControllerState::Disabled { .. },
                LossyWindowControllerEffect::AckReceived(_),
            ) => None,
            _ => {
                panic!("Unexpected toggle!")
            }
        })
        .map(|x| {
            self.sender
                .create_message(LossySenderEffect::SettingsUpdate(x))
        })
        .into_iter()
        .collect_vec()
    }
}

#[derive(Debug)]
struct WaitingForEnable {
    packets_sent: u64,
    average_throughput: DisabledInfoRateMeter,
    average_rtt: Mean<TimeSpan>,
}

#[derive(Debug)]
struct Enabled {
    last_send: Time,
    greatest_ack: u64,
    settings: LossyWindowSettings,
    packets_sent: u64,
    average_throughput: EnabledInfoRateMeter,
    average_rtt: Mean<TimeSpan>,
}

impl Enabled {
    const fn new(
        settings: LossyWindowSettings,
        packets_sent: u64,
        average_throughput: EnabledInfoRateMeter,
        average_rtt: Mean<TimeSpan>,
    ) -> Self {
        Self {
            last_send: Time::MIN,
            greatest_ack: 0,
            settings,
            packets_sent,
            average_throughput,
            average_rtt,
        }
    }

    fn next_send(&self, time: Time) -> Option<Time> {
        if self.packets_sent < self.greatest_ack + u64::from(self.settings.window) {
            Some(latest(&[
                self.last_send + self.settings.intersend_delay,
                time,
            ]))
        } else {
            None
        }
    }
}

#[derive(Debug, From)]
enum LossyWindowState {
    WaitingForEnable(WaitingForEnable),
    Enabled(Enabled),
}

#[derive_where(Debug; L)]
pub struct LossyWindowSender<'sim, 'a, E, L> {
    controller: MessageDestination<'sim, LossyWindowControllerEffect, E>,
    id: PacketDestination<'sim, E>,
    link: PacketDestination<'sim, E>,
    destination: PacketDestination<'sim, E>,
    state: LossyWindowState,
    logger: L,
    phantom: PhantomData<&'a ()>,
}

impl<'sim, 'a, E, L> LossyWindowSender<'sim, 'a, E, L>
where
    L: Logger,
{
    pub fn new(
        controller: MessageDestination<'sim, LossyWindowControllerEffect, E>,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        logger: L,
    ) -> LossyWindowSender<'sim, 'a, E, L> {
        LossyWindowSender {
            controller,
            id,
            link,
            destination,
            state: WaitingForEnable {
                packets_sent: 0,
                average_throughput: DisabledInfoRateMeter::new(),
                average_rtt: Mean::new(),
            }
            .into(),
            logger,
            phantom: PhantomData,
        }
    }

    fn receive_packet(
        &mut self,
        packet: &Packet<'sim, E>,
        EffectContext { time, .. }: EffectContext,
    ) -> Vec<Message<'sim, E>> {
        match &mut self.state {
            LossyWindowState::WaitingForEnable(_) => {
                log!(
                    self.logger,
                    "Received packet {}, ignoring as disabled",
                    packet.seq
                );
                vec![]
            }
            LossyWindowState::Enabled(Enabled {
                settings,
                greatest_ack,
                average_rtt,
                average_throughput,
                ..
            }) => {
                average_rtt.record(time - packet.sent_time);
                average_throughput.record_info(packet.size());
                log!(self.logger, "Received packet {}", packet.seq);
                *greatest_ack = (*greatest_ack).max(packet.seq);
                vec![self
                    .controller
                    .create_message(LossyWindowControllerEffect::AckReceived(AckReceived {
                        current_settings: settings.clone(),
                        sent_time: packet.sent_time,
                        received_time: time,
                    }))]
            }
        }
    }

    fn receive_settings_update(
        &mut self,
        settings: SettingsUpdate,
        EffectContext { time, .. }: EffectContext,
    ) {
        match (&mut self.state, settings) {
            (
                LossyWindowState::WaitingForEnable(WaitingForEnable {
                    packets_sent,
                    average_throughput,
                    average_rtt,
                }),
                SettingsUpdate::Enable(settings),
            ) => {
                log!(self.logger, "Enabled");
                self.state = Enabled::new(
                    settings,
                    *packets_sent,
                    average_throughput.clone().enable(time),
                    average_rtt.clone(),
                )
                .into();
            }
            (
                LossyWindowState::Enabled(Enabled { settings, .. }),
                SettingsUpdate::Enable(new_settings),
            ) => {
                log!(self.logger, "Updated settings");
                *settings = new_settings;
            }
            (
                LossyWindowState::Enabled(Enabled {
                    packets_sent,
                    average_throughput,
                    average_rtt,
                    ..
                }),
                SettingsUpdate::Disable,
            ) => {
                log!(self.logger, "Disabled");
                self.state = WaitingForEnable {
                    packets_sent: *packets_sent,
                    average_throughput: average_throughput.clone().disable(time),
                    average_rtt: average_rtt.clone(),
                }
                .into();
            }
            _ => {}
        }
    }

    fn send(&mut self, EffectContext { time, .. }: EffectContext) -> Packet<'sim, E> {
        match &mut self.state {
            LossyWindowState::Enabled(Enabled {
                packets_sent,
                last_send,
                ..
            }) => {
                *packets_sent += 1;
                *last_send = time;
                Packet {
                    seq: *packets_sent,
                    source: self.id.clone(),
                    destination: self.destination.clone(),
                    sent_time: time,
                }
            }
            LossyWindowState::WaitingForEnable(_) => panic!(),
        }
    }

    fn average_throughput(
        &self,
        current_time: Time,
    ) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
        match &self.state {
            LossyWindowState::WaitingForEnable(WaitingForEnable {
                average_throughput, ..
            }) => average_throughput.current_value(),
            LossyWindowState::Enabled(Enabled {
                average_throughput, ..
            }) => average_throughput.current_value(current_time),
        }
    }

    fn average_rtt(&self) -> Result<TimeSpan, NoPacketsAcked> {
        match &self.state {
            LossyWindowState::WaitingForEnable(WaitingForEnable { average_rtt, .. })
            | LossyWindowState::Enabled(Enabled { average_rtt, .. }) => {
                average_rtt.value().map_err(|_| NoPacketsAcked)
            }
        }
    }
}

enum SettingsUpdate {
    Enable(LossyWindowSettings),
    Disable,
}

#[derive(From, TryInto)]
pub enum LossySenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    #[allow(private_interfaces)]
    SettingsUpdate(SettingsUpdate),
}

impl<'sim, 'a, E, L> Component<'sim, E> for LossyWindowSender<'sim, 'a, E, L>
where
    L: Logger,
{
    type Receive = LossySenderEffect<'sim, E>;

    fn next_tick(&self, time: Time) -> Option<Time> {
        match &self.state {
            LossyWindowState::WaitingForEnable(_) => None,
            LossyWindowState::Enabled(enabled) => enabled.next_send(time),
        }
    }

    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>> {
        let time = context.time;
        assert_eq!(Some(time), Component::next_tick(self, time));
        let packet = self.send(context);
        vec![self.link.create_message(packet)]
    }

    fn receive(&mut self, e: Self::Receive, context: EffectContext) -> Vec<Message<'sim, E>> {
        match e {
            LossySenderEffect::Packet(packet) => self.receive_packet(&packet, context),
            LossySenderEffect::SettingsUpdate(update) => {
                self.receive_settings_update(update, context);
                vec![]
            }
        }
    }
}

#[derive_where(Clone)]
pub struct LossySenderDestinations<'sim, E> {
    pub packet_destination: MessageDestination<'sim, Packet<'sim, E>, E>,
    pub toggle_destination: MessageDestination<'sim, Toggle, E>,
}

pub struct LossyWindowSenderSlot<'sim, 'a, 'b, E> {
    sender_slot: ComponentSlot<'sim, 'a, 'b, LossySenderEffect<'sim, E>, E>,
    controller_slot: ComponentSlot<'sim, 'a, 'b, LossyWindowControllerEffect, E>,
    destinations: LossySenderDestinations<'sim, E>,
}

impl<'sim, 'a, 'b, E> LossyWindowSenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<LossyWindowControllerEffect> + HasSubEffect<LossySenderEffect<'sim, E>> + 'sim,
{
    #[must_use]
    pub fn destination(&self) -> LossySenderDestinations<'sim, E> {
        self.destinations.clone()
    }

    pub fn set<B>(
        self,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: impl Logger + Clone + 'a,
    ) -> (LossySenderDestinations<'sim, E>, Rc<dyn Flow + 'a>)
    where
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let LossyWindowSenderSlot {
            sender_slot,
            controller_slot,
            destinations,
        } = self;
        let sender = Rc::new(RefCell::new(LossyWindowSender::new(
            controller_slot.destination(),
            id,
            link,
            destination,
            logger.clone(),
        )));
        let sender_destination = sender_slot.set(DynComponent::shared(sender.clone()
            as Rc<RefCell<dyn Component<'sim, E, Receive = LossySenderEffect<'sim, E>> + 'a>>));
        controller_slot.set(DynComponent::new(LossyWindowController::new(
            sender_destination,
            new_behavior,
            wait_for_enable,
            logger,
        )));
        (destinations, sender)
    }
}

impl<'sim, 'a, E, L> LossyWindowSender<'sim, 'a, E, L>
where
    E: HasSubEffect<LossySenderEffect<'sim, E>> + HasSubEffect<LossyWindowControllerEffect> + 'sim,
    L: Logger + Clone,
{
    pub fn reserve_slot<'b>(
        builder: &'b SimulatorBuilder<'sim, 'a, E>,
    ) -> LossyWindowSenderSlot<'sim, 'a, 'b, E> {
        let sender_slot = builder.reserve_slot();
        let controller_slot = builder.reserve_slot();
        LossyWindowSenderSlot {
            destinations: LossySenderDestinations {
                packet_destination: sender_slot.destination().cast(),
                toggle_destination: controller_slot.destination().cast(),
            },
            sender_slot,
            controller_slot,
        }
    }

    pub fn insert<B>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> (
        LossySenderDestinations<'sim, E>,
        Rc<dyn Flow + 'a>,
    )
    where
        L: 'a,
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot(builder);
        slot.set(id, link, destination, new_behavior, wait_for_enable, logger)
        //(builder.insert(LossyWindowSender::new(id, link, destination, wait_for_enable, logger)), builder.insert(LossyWin))
    }
}

impl<'sim, 'a, E, L> Flow for LossyWindowSender<'sim, 'a, E, L>
where
    L: Logger,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.average_throughput(current_time)
            .map_err(|_| FlowNeverActive {})
            .map(|average_throughput| FlowProperties {
                average_throughput,
                average_rtt: self.average_rtt(),
            })
    }
}

#[derive_where(Debug; L)]
pub struct LossyBouncer<'sim, E, L> {
    link: PacketDestination<'sim, E>,
    logger: L,
}

impl<'sim, E, L> LossyBouncer<'sim, E, L> {
    pub const fn new(link: PacketDestination<'sim, E>, logger: L) -> LossyBouncer<E, L> {
        LossyBouncer { link, logger }
    }
}

impl<'sim, E, L> Component<'sim, E> for LossyBouncer<'sim, E, L>
where
    L: Logger,
{
    type Receive = Packet<'sim, E>;

    fn tick(&mut self, _: EffectContext) -> Vec<Message<'sim, E>> {
        vec![]
    }

    fn receive(&mut self, packet: Self::Receive, _: EffectContext) -> Vec<Message<'sim, E>> {
        let seq = packet.seq;
        let message = self.link.create_message(Packet {
            source: packet.destination,
            destination: packet.source,
            ..packet
        });
        log!(
            self.logger,
            "Bouncing packet {} via {:?}",
            seq,
            message.destination()
        );
        vec![message]
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        None
    }
}
