use std::fmt::Debug;

use derive_more::{From, TryInto};
use derive_where::derive_where;

use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties, NoPacketsAcked},
    logging::Logger,
    meters::{DisabledInfoRateMeter, EnabledInfoRateMeter, InfoRateMeterNeverEnabled, Mean},
    network::{toggler::Toggle, Packet, PacketDestination},
    quantities::{latest, InformationRate, Time, TimeSpan},
    simulation::{Component, EffectContext, Message},
};

#[derive(Debug)]
pub struct LossyWindowSettings {
    pub window: u32,
    pub intersend_delay: TimeSpan,
}

pub trait LossyWindowBehavior<'a, L>: Debug {
    fn initial_settings(&self) -> LossyWindowSettings;
    fn ack_received(
        &mut self,
        settings: &mut LossyWindowSettings,
        sent_time: Time,
        received_time: Time,
        logger: &mut L,
    );
}

#[derive(Debug)]
struct WaitingForEnable {
    packets_sent: u64,
    average_throughput: DisabledInfoRateMeter,
    average_rtt: Mean<TimeSpan>,
}

#[derive(Debug)]
struct Enabled<B> {
    last_send: Time,
    greatest_ack: u64,
    settings: LossyWindowSettings,
    packets_sent: u64,
    behavior: B,
    average_throughput: EnabledInfoRateMeter,
    average_rtt: Mean<TimeSpan>,
}

impl<B> Enabled<B> {
    fn new<'a, L>(
        behavior: B,
        packets_sent: u64,
        average_throughput: EnabledInfoRateMeter,
        average_rtt: Mean<TimeSpan>,
    ) -> Self
    where
        B: LossyWindowBehavior<'a, L>,
    {
        Self {
            last_send: Time::MIN,
            greatest_ack: 0,
            settings: behavior.initial_settings(),
            packets_sent,
            behavior,
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

#[derive(Debug)]
enum LossyWindowState<B> {
    WaitingForEnable(WaitingForEnable),
    Enabled(Enabled<B>),
}

impl<B> From<WaitingForEnable> for LossyWindowState<B> {
    fn from(value: WaitingForEnable) -> Self {
        LossyWindowState::WaitingForEnable(value)
    }
}

impl<B> From<Enabled<B>> for LossyWindowState<B> {
    fn from(value: Enabled<B>) -> Self {
        LossyWindowState::Enabled(value)
    }
}

pub struct LossyWindowSender<'sim, 'a, B, E, L> {
    new_behavior: Box<dyn (Fn() -> B) + 'a>,
    id: PacketDestination<'sim, E>,
    link: PacketDestination<'sim, E>,
    destination: PacketDestination<'sim, E>,
    state: LossyWindowState<B>,
    logger: L,
}

impl<'sim, 'a, B, E, L> Debug for LossyWindowSender<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior<'a, L>,
    L: Logger,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossyWindowSender")
            .field("id", &self.id)
            .field("link", &self.link)
            .field("destination", &self.destination)
            .field("state", &self.state)
            .field("logger", &self.logger)
            .finish_non_exhaustive()
    }
}

impl<'sim, 'a, B, E, L> LossyWindowSender<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior<'a, L>,
    L: Logger,
{
    pub fn new(
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> LossyWindowSender<'sim, 'a, B, E, L> {
        LossyWindowSender {
            id,
            link,
            destination,
            state: if wait_for_enable {
                WaitingForEnable {
                    packets_sent: 0,
                    average_throughput: DisabledInfoRateMeter::new(),
                    average_rtt: Mean::new(),
                }
                .into()
            } else {
                Enabled::new(
                    new_behavior(),
                    0,
                    EnabledInfoRateMeter::new(Time::SIM_START),
                    Mean::new(),
                )
                .into()
            },
            new_behavior,
            logger,
        }
    }

    fn receive_packet(
        &mut self,
        packet: &Packet<'sim, E>,
        EffectContext { time, .. }: EffectContext,
    ) {
        match &mut self.state {
            LossyWindowState::WaitingForEnable(_) => {
                log!(
                    self.logger,
                    "Received packet {}, ignoring as disabled",
                    packet.seq
                );
            }
            LossyWindowState::Enabled(Enabled {
                behavior,
                settings,
                greatest_ack,
                average_rtt,
                average_throughput,
                ..
            }) => {
                average_rtt.record(time - packet.sent_time);
                average_throughput.record_info(packet.size());
                behavior.ack_received(settings, packet.sent_time, time, &mut self.logger);
                log!(self.logger, "Received packet {}", packet.seq);
                *greatest_ack = (*greatest_ack).max(packet.seq);
            }
        }
    }

    fn receive_toggle(&mut self, toggle: Toggle, EffectContext { time, .. }: EffectContext) {
        match (&self.state, toggle) {
            (
                LossyWindowState::WaitingForEnable(WaitingForEnable {
                    packets_sent,
                    average_throughput,
                    average_rtt,
                }),
                Toggle::Enable,
            ) => {
                log!(self.logger, "Enabled");
                self.state = Enabled::new(
                    (self.new_behavior)(),
                    *packets_sent,
                    average_throughput.clone().enable(time),
                    average_rtt.clone(),
                )
                .into();
            }
            (
                LossyWindowState::Enabled(Enabled {
                    packets_sent,
                    average_throughput,
                    average_rtt,
                    ..
                }),
                Toggle::Disable,
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

#[derive(From, TryInto)]
pub enum LossySenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    Toggle(Toggle),
}

impl<'sim, 'a, B, E, L> Component<'sim, E> for LossyWindowSender<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior<'a, L>,
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
            LossySenderEffect::Toggle(toggle) => self.receive_toggle(toggle, context),
        }
        vec![]
    }
}

impl<'sim, 'a, B, E, L> Flow for LossyWindowSender<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior<'a, L>,
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
        log!(
            self.logger,
            "Bounced packet {} back to {:?}",
            packet.seq,
            packet.source
        );
        vec![self.link.create_message(Packet {
            source: packet.destination,
            destination: packet.source,
            ..packet
        })]
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        None
    }
}
