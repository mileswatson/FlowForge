use std::fmt::Debug;

use crate::{
    average::{DisabledRateMeter, EnabledRateMeter, Mean, RateMeterNeverEnabled},
    flow::{Flow, FlowNeverActive, FlowProperties, NoPacketsAcked},
    logging::Logger,
    network::{link::Routable, toggler::Toggle},
    simulation::{
        try_case, Component, ComponentId, EffectContext, HasVariant, MaybeHasVariant, Message,
    },
    time::{latest, Rate, Time, TimeSpan},
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
    average_throughput: DisabledRateMeter,
    average_rtt: Mean<TimeSpan>,
}

#[derive(Debug)]
struct Enabled<B> {
    last_send: Time,
    greatest_ack: u64,
    settings: LossyWindowSettings,
    packets_sent: u64,
    behavior: B,
    average_throughput: EnabledRateMeter,
    average_rtt: Mean<TimeSpan>,
}

impl<B> Enabled<B> {
    fn new<'a, L>(
        behavior: B,
        packets_sent: u64,
        average_throughput: EnabledRateMeter,
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

#[derive(Debug)]
pub struct Packet {
    seq: u64,
    source: ComponentId,
    destination: ComponentId,
    sent_time: Time,
}

impl Routable for Packet {
    fn pop_next_hop(&mut self) -> ComponentId {
        self.destination
    }
}

pub struct LossyWindowSender<'a, B, L> {
    new_behavior: Box<dyn (Fn() -> B) + 'a>,
    id: ComponentId,
    link: ComponentId,
    destination: ComponentId,
    state: LossyWindowState<B>,
    logger: L,
}

impl<'a, B, L> Debug for LossyWindowSender<'a, B, L>
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

impl<'a, B, L> LossyWindowSender<'a, B, L>
where
    B: LossyWindowBehavior<'a, L>,
    L: Logger,
{
    pub fn new(
        id: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> LossyWindowSender<'a, B, L> {
        LossyWindowSender {
            id,
            link,
            destination,
            state: if wait_for_enable {
                WaitingForEnable {
                    packets_sent: 0,
                    average_throughput: DisabledRateMeter::new(),
                    average_rtt: Mean::new(),
                }
                .into()
            } else {
                Enabled::new(
                    new_behavior(),
                    0,
                    EnabledRateMeter::new(Time::sim_start()),
                    Mean::new(),
                )
                .into()
            },
            new_behavior,
            logger,
        }
    }

    fn receive_packet(&mut self, packet: &Packet, EffectContext { time, .. }: EffectContext) {
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
                average_throughput.record_event();
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

    fn send(&mut self, EffectContext { time, .. }: EffectContext) -> Packet {
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
                    source: self.id,
                    destination: self.destination,
                    sent_time: time,
                }
            }
            LossyWindowState::WaitingForEnable(_) => panic!(),
        }
    }

    fn average_throughput(&self, current_time: Time) -> Result<Rate, RateMeterNeverEnabled> {
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
                average_rtt.value().ok_or(NoPacketsAcked {})
            }
        }
    }
}

impl<'a, E, B, L> Component<E> for LossyWindowSender<'a, B, L>
where
    E: HasVariant<Packet> + MaybeHasVariant<Toggle>,
    B: LossyWindowBehavior<'a, L>,
    L: Logger,
{
    fn next_tick(&self, time: Time) -> Option<Time> {
        match &self.state {
            LossyWindowState::WaitingForEnable(_) => None,
            LossyWindowState::Enabled(enabled) => enabled.next_send(time),
        }
    }

    fn tick(&mut self, context: EffectContext) -> Vec<Message<E>> {
        let time = context.time;
        assert_eq!(Some(time), Component::<E>::next_tick(self, time));
        let packet = self.send(context);
        vec![Message {
            component_id: self.link,
            effect: packet.into(),
        }]
    }

    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<E>> {
        e.try_case(context, |packet, ctx| self.receive_packet(&packet, ctx))
            .or_else(try_case(|toggle, ctx| self.receive_toggle(toggle, ctx)))
            .unwrap();
        vec![]
    }
}

impl<'a, B, L> Flow for LossyWindowSender<'a, B, L>
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

#[derive(Debug)]
pub struct LossyBouncer<L> {
    link: ComponentId,
    logger: L,
}

impl<L> LossyBouncer<L> {
    pub const fn new(link: ComponentId, logger: L) -> LossyBouncer<L> {
        LossyBouncer { link, logger }
    }
}

impl<E, L> Component<E> for LossyBouncer<L>
where
    E: HasVariant<Packet>,
    L: Logger,
{
    fn tick(&mut self, _: EffectContext) -> Vec<Message<E>> {
        vec![]
    }

    fn receive(&mut self, e: E, _: EffectContext) -> Vec<Message<E>> {
        let packet = e.try_into().unwrap();
        log!(
            self.logger,
            "Bounced packet {} back to {:?}",
            packet.seq,
            packet.source
        );
        vec![Message {
            component_id: self.link,
            effect: Packet {
                source: packet.destination,
                destination: packet.source,
                ..packet
            }
            .into(),
        }]
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        None
    }
}
