use std::{fmt::Debug, marker::PhantomData};

use derive_more::From;
use derive_where::derive_where;

use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties, NoPacketsAcked},
    logging::Logger,
    meters::{DisabledInfoRateMeter, EnabledInfoRateMeter, InfoRateMeterNeverEnabled, Mean},
    network::{Packet, PacketAddress},
    quantities::{latest, InformationRate, Time, TimeSpan},
    simulation::{Address, Component, EffectContext, Message},
};

use super::{
    AckReceived, LossyInternalControllerEffect, LossyInternalSenderEffect, LossyWindowSettings,
    SettingsUpdate,
};

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
enum State {
    WaitingForEnable(WaitingForEnable),
    Enabled(Enabled),
}

#[derive_where(Debug; L)]
pub struct Sender<'sim, 'a, E, L> {
    controller: Address<'sim, LossyInternalControllerEffect, E>,
    id: PacketAddress<'sim, E>,
    link: PacketAddress<'sim, E>,
    destination: PacketAddress<'sim, E>,
    state: State,
    logger: L,
    phantom: PhantomData<&'a ()>,
}

impl<'sim, 'a, E, L> Sender<'sim, 'a, E, L>
where
    L: Logger,
{
    pub fn new(
        controller: Address<'sim, LossyInternalControllerEffect, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        logger: L,
    ) -> Sender<'sim, 'a, E, L> {
        Sender {
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
            State::WaitingForEnable(_) => {
                log!(
                    self.logger,
                    "Received packet {}, ignoring as disabled",
                    packet.seq
                );
                vec![]
            }
            State::Enabled(Enabled {
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
                    .create_message(LossyInternalControllerEffect::AckReceived(AckReceived {
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
                State::WaitingForEnable(WaitingForEnable {
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
            (State::Enabled(Enabled { settings, .. }), SettingsUpdate::Enable(new_settings)) => {
                log!(self.logger, "Updated settings");
                *settings = new_settings;
            }
            (
                State::Enabled(Enabled {
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
            State::Enabled(Enabled {
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
            State::WaitingForEnable(_) => panic!(),
        }
    }

    fn average_throughput(
        &self,
        current_time: Time,
    ) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
        match &self.state {
            State::WaitingForEnable(WaitingForEnable {
                average_throughput, ..
            }) => average_throughput.current_value(),
            State::Enabled(Enabled {
                average_throughput, ..
            }) => average_throughput.current_value(current_time),
        }
    }

    fn average_rtt(&self) -> Result<TimeSpan, NoPacketsAcked> {
        match &self.state {
            State::WaitingForEnable(WaitingForEnable { average_rtt, .. })
            | State::Enabled(Enabled { average_rtt, .. }) => {
                average_rtt.value().map_err(|_| NoPacketsAcked)
            }
        }
    }
}

impl<'sim, 'a, E, L> Component<'sim, E> for Sender<'sim, 'a, E, L>
where
    L: Logger,
{
    type Receive = LossyInternalSenderEffect<'sim, E>;

    fn next_tick(&self, time: Time) -> Option<Time> {
        match &self.state {
            State::WaitingForEnable(_) => None,
            State::Enabled(enabled) => enabled.next_send(time),
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
            LossyInternalSenderEffect::Packet(packet) => self.receive_packet(&packet, context),
            LossyInternalSenderEffect::SettingsUpdate(update) => {
                self.receive_settings_update(update, context);
                vec![]
            }
        }
    }
}

impl<'sim, 'a, E, L> Flow for Sender<'sim, 'a, E, L>
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
