use std::{fmt::Debug, marker::PhantomData};

use derive_more::From;
use derive_where::derive_where;

use crate::{
    components::packet::{Packet, PacketAddress},
    core::{logging::Logger, meters::FlowMeter},
    quantities::{latest, Time},
    simulation::{Address, Component, EffectContext, Message},
};

use super::{
    AckReceived, CwndSettings, LossyInternalControllerEffect, LossyInternalSenderEffect,
    SettingsUpdate,
};

#[derive(Debug)]
struct WaitingForEnable {
    packets_sent: u64,
}

#[derive(Debug)]
struct Enabled {
    last_send: Time,
    greatest_ack: u64,
    settings: CwndSettings,
    packets_sent: u64,
}

impl Enabled {
    const fn new(settings: CwndSettings, packets_sent: u64) -> Self {
        Self {
            last_send: Time::MIN,
            greatest_ack: packets_sent,
            settings,
            packets_sent,
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

#[derive_where(Debug; F, L)]
pub struct Sender<'sim, 'a, F, E, L> {
    controller: Address<'sim, LossyInternalControllerEffect, E>,
    id: PacketAddress<'sim, E>,
    link: PacketAddress<'sim, E>,
    destination: PacketAddress<'sim, E>,
    state: State,
    flow_meter: F,
    logger: L,
    phantom: PhantomData<&'a ()>,
}

impl<'sim, 'a, F, E, L> Sender<'sim, 'a, F, E, L>
where
    F: FlowMeter,
    L: Logger,
{
    pub fn new(
        controller: Address<'sim, LossyInternalControllerEffect, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        flow_meter: F,
        logger: L,
    ) -> Sender<'sim, 'a, F, E, L> {
        Sender {
            controller,
            id,
            link,
            destination,
            state: WaitingForEnable { packets_sent: 0 }.into(),
            flow_meter,
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
                ..
            }) => {
                self.flow_meter
                    .packet_received(packet.size(), time - packet.sent_time, time);
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
                State::WaitingForEnable(WaitingForEnable { packets_sent }),
                SettingsUpdate::Enable(settings),
            ) => {
                log!(self.logger, "Enabled");
                self.flow_meter.flow_enabled(time);
                self.state = Enabled::new(settings, *packets_sent).into();
            }
            (State::Enabled(Enabled { settings, .. }), SettingsUpdate::Enable(new_settings)) => {
                log!(self.logger, "Updated settings");
                *settings = new_settings;
            }
            (State::Enabled(Enabled { packets_sent, .. }), SettingsUpdate::Disable) => {
                self.flow_meter.flow_disabled(time);
                log!(self.logger, "Disabled");
                self.state = WaitingForEnable {
                    packets_sent: *packets_sent,
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
}

impl<'sim, 'a, F, E, L> Component<'sim, E> for Sender<'sim, 'a, F, E, L>
where
    F: FlowMeter + 'a,
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
