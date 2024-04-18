use std::{fmt::Debug, marker::PhantomData};

use derive_more::{From, TryInto};

use crate::{
    components::{
        packet::{Packet, PacketAddress},
        toggler::Toggle,
    },
    quantities::{earliest_opt, Time},
    simulation::{Component, EffectContext, Message},
    util::{logging::Logger, meters::FlowMeter, rand::Rng},
    AckReceived, Cca, PacketSent,
};

#[derive(Debug)]
struct Disabled {
    packets_sent: u64,
}

#[derive(Debug)]
struct Enabled<C> {
    started: Time,
    last_send: Time,
    greatest_ack: u64,
    cwnd: u32,
    packets_sent: u64,
    cca: C,
}

impl<C: Cca> Enabled<C> {
    fn new(cca: C, packets_sent: u64, time: Time) -> Self {
        Self {
            started: time,
            last_send: Time::MIN,
            greatest_ack: packets_sent,
            cwnd: cca.initial_cwnd(time),
            cca,
            packets_sent,
        }
    }

    fn next_send(&self, time: Time) -> Option<Time> {
        if self.packets_sent < self.greatest_ack + u64::from(self.cwnd) {
            Some(time)
        } else {
            None
        }
    }
}

#[derive(Debug, From)]
enum State<C> {
    WaitingForEnable(Disabled),
    Enabled(Enabled<C>),
}

#[derive(From, TryInto)]
pub enum LossySenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    Toggle(Toggle),
}

pub struct LossySender<'sim, 'a, C, F, G, E, L> {
    id: PacketAddress<'sim, E>,
    link: PacketAddress<'sim, E>,
    destination: PacketAddress<'sim, E>,
    cca_generator: G,
    state: State<C>,
    flow_meter: F,
    rng: Rng,
    logger: L,
    phantom: PhantomData<&'a ()>,
}

impl<'sim, 'a, C: Debug, F: Debug, G, E, L: Debug> Debug for LossySender<'sim, 'a, C, F, G, E, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossySender")
            .field("id", &self.id)
            .field("link", &self.link)
            .field("destination", &self.destination)
            .field("state", &self.state)
            .field("flow_meter", &self.flow_meter)
            .field("rng", &self.rng)
            .field("logger", &self.logger)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<'sim, 'a, C, F, G, E, L> LossySender<'sim, 'a, C, F, G, E, L>
where
    C: Cca,
    G: Fn() -> C,
    F: FlowMeter,
    L: Logger,
{
    pub fn new(
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        mut flow_meter: F,
        cca_generator: G,
        wait_for_enable: bool,
        rng: Rng,
        logger: L,
    ) -> Self {
        if !wait_for_enable {
            flow_meter.flow_enabled(Time::SIM_START);
        }
        LossySender {
            id,
            link,
            destination,
            state: if wait_for_enable {
                Disabled { packets_sent: 0 }.into()
            } else {
                Enabled::new(cca_generator(), 0, Time::SIM_START).into()
            },
            cca_generator,
            rng,
            flow_meter,
            logger,
            phantom: PhantomData,
        }
    }

    fn receive_toggle(&mut self, toggle: Toggle, EffectContext { time, .. }: EffectContext) {
        match (&mut self.state, toggle) {
            (State::WaitingForEnable(Disabled { packets_sent, .. }), Toggle::Enable) => {
                log!(self.logger, "Enabled");
                self.flow_meter.flow_enabled(time);
                self.state = Enabled::new((self.cca_generator)(), *packets_sent, time).into();
            }
            (State::Enabled(Enabled { packets_sent, .. }), Toggle::Disable) => {
                log!(self.logger, "Disabled");
                self.flow_meter.flow_disabled(time);
                self.state = Disabled {
                    packets_sent: *packets_sent,
                }
                .into();
            }
            _ => panic!("Unexpected toggle!"),
        }
    }

    fn receive_packet(
        &mut self,
        packet: &Packet<'sim, E>,
        EffectContext { time, .. }: EffectContext,
    ) {
        match &mut self.state {
            State::WaitingForEnable(_) => {
                log!(
                    self.logger,
                    "Received packet {}, ignoring as disabled",
                    packet.seq
                );
            }
            State::Enabled(Enabled {
                started,
                greatest_ack,
                cwnd,
                cca,
                ..
            }) => {
                if &packet.sent_time < started {
                    log!(self.logger, "Received old packet {}", packet.seq);
                    return;
                }
                self.flow_meter
                    .packet_received(packet.size(), time - packet.sent_time, time);
                log!(self.logger, "Received packet {}", packet.seq);
                *cwnd = cca.ack_received(
                    AckReceived {
                        sent_time: packet.sent_time,
                        received_time: time,
                    },
                    &mut self.rng,
                    &mut self.logger,
                );
                *greatest_ack = (*greatest_ack).max(packet.seq);
            }
        }
    }
}

impl<'sim, 'a, C, F, G, E, L> Component<'sim, E> for LossySender<'sim, 'a, C, F, G, E, L>
where
    C: Cca,
    G: Fn() -> C,
    F: FlowMeter + 'a,
    L: Logger,
{
    type Receive = LossySenderEffect<'sim, E>;

    fn next_tick(&self, time: Time) -> Option<Time> {
        match &self.state {
            State::WaitingForEnable(_) => None,
            State::Enabled(enabled) => {
                earliest_opt(&[enabled.next_send(time), enabled.cca.next_tick(time)])
            }
        }
    }

    fn tick(&mut self, EffectContext { time }: EffectContext) -> Vec<Message<'sim, E>> {
        match &mut self.state {
            State::Enabled(s) => {
                if s.cca.next_tick(time) == Some(time) {
                    s.cwnd = s.cca.tick(&mut self.rng, &mut self.logger);
                    vec![]
                } else if s.next_send(time) == Some(time) {
                    let Enabled {
                        last_send,
                        cwnd,
                        packets_sent,
                        cca,
                        ..
                    } = s;
                    *packets_sent += 1;
                    *last_send = time;
                    let packet = Packet {
                        seq: *packets_sent,
                        source: self.id.clone(),
                        destination: self.destination.clone(),
                        sent_time: time,
                    };
                    *cwnd = cca.packet_sent(
                        PacketSent { sent_time: time },
                        &mut self.rng,
                        &mut self.logger,
                    );
                    vec![self.link.create_message(packet)]
                } else {
                    panic!()
                }
            }
            State::WaitingForEnable(_) => panic!(),
        }
    }

    fn receive(&mut self, e: Self::Receive, ctx: EffectContext) -> Vec<Message<'sim, E>> {
        match e {
            LossySenderEffect::Packet(packet) => self.receive_packet(&packet, ctx),
            LossySenderEffect::Toggle(toggle) => self.receive_toggle(toggle, ctx),
        }
        vec![]
    }
}
