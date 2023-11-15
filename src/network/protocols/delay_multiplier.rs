use crate::{
    logging::Logger,
    network::link::Routable,
    simulation::{Component, ComponentId, EffectContext, EffectResult, HasVariant, Message},
    time::{Float, Time, TimeSpan},
};

#[derive(Debug)]
pub struct Packet {
    seq: u64,
    source: ComponentId,
    destination: ComponentId,
}

impl Routable for Packet {
    fn pop_next_hop(&mut self) -> ComponentId {
        self.destination
    }
}

#[derive(Debug)]
enum SenderState {
    Starting,
    WaitingForAck {
        first_send_time: Option<Time>,
        next_send_time: Time,
        resend_time: Time,
        current_seq: u64,
        timeout: TimeSpan,
        exp_average_rtt: TimeSpan,
    },
    WaitingForSend {
        send_time: Time,
        current_seq: u64,
        timeout: TimeSpan,
        exp_average_rtt: TimeSpan,
    },
}

#[derive(Debug)]
pub struct Sender<L> {
    index: ComponentId,
    link: ComponentId,
    destination: ComponentId,
    delay_multiplier: Float,
    start_delay: TimeSpan,
    state: SenderState,
    logger: L,
}

impl<'a, L> Sender<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub const fn new<E>(
        index: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        delay_multiplier: Float,
        start_delay: TimeSpan,
        logger: L,
    ) -> Sender<L>
    where
        E: HasVariant<Packet>,
    {
        Sender {
            index,
            link,
            destination,
            delay_multiplier,
            start_delay,
            state: SenderState::Starting,
            logger,
        }
    }

    fn send<E: HasVariant<Packet>>(&mut self, seq: u64, next_tick: Time) -> EffectResult<E> {
        log!(self.logger, "Sending packet {}", seq);
        EffectResult {
            next_tick: Some(next_tick),
            effects: vec![Message::new(
                self.link,
                Packet {
                    seq,
                    source: self.index,
                    destination: self.destination,
                },
            )],
        }
    }

    fn next_send_time(&self, time: Time, exp_average_rtt: TimeSpan) -> Time {
        time + self.delay_multiplier * exp_average_rtt
    }

    pub const fn packets(&self) -> u64 {
        match self.state {
            SenderState::Starting => 0,
            SenderState::WaitingForAck { current_seq, .. }
            | SenderState::WaitingForSend { current_seq, .. } => current_seq,
        }
    }
}

fn new_exp_average(exp_average_rtt: TimeSpan, ts: TimeSpan) -> TimeSpan {
    const ALPHA: f64 = 0.8;
    ALPHA * exp_average_rtt + (1. - ALPHA) * ts
}

fn next_resend_time(time: Time, timeout: TimeSpan, next_send_time: Time) -> Time {
    let timeout = time + timeout;
    if timeout > next_send_time {
        timeout
    } else {
        next_send_time
    }
}

const fn ignore<E>(next_tick: Time) -> EffectResult<E> {
    EffectResult {
        next_tick: Some(next_tick),
        effects: Vec::new(),
    }
}

impl<'a, E, L> Component<E> for Sender<L>
where
    E: HasVariant<Packet>,
    L: Logger + 'a,
{
    fn tick(&mut self, EffectContext { time, .. }: EffectContext) -> EffectResult<E> {
        match self.state {
            SenderState::Starting => {
                if time == Time::sim_start() + self.start_delay {
                    let current_seq = 0;
                    let timeout = TimeSpan::new(1.0);
                    let exp_average_rtt = TimeSpan::new(0.5);
                    let next_send_time = self.next_send_time(time, exp_average_rtt);
                    let resend_time = next_resend_time(time, timeout, next_send_time);
                    self.state = SenderState::WaitingForAck {
                        first_send_time: Some(time),
                        current_seq,
                        timeout,
                        exp_average_rtt,
                        next_send_time,
                        resend_time,
                    };
                    self.send(current_seq, resend_time)
                } else {
                    ignore(Time::sim_start() + self.start_delay)
                }
            }
            SenderState::WaitingForAck {
                resend_time,
                current_seq,
                timeout,
                exp_average_rtt,
                ..
            } => {
                assert_eq!(time, resend_time);
                let exp_average_rtt = new_exp_average(exp_average_rtt, timeout);
                let timeout = 2. * timeout;
                log!(self.logger, "Timed out, so adjusted timeout to {}", timeout);
                let next_send_time = self.next_send_time(time, exp_average_rtt);
                let resend_time = next_resend_time(time, timeout, next_send_time);
                self.state = SenderState::WaitingForAck {
                    first_send_time: None,
                    next_send_time,
                    resend_time,
                    current_seq,
                    timeout,
                    exp_average_rtt,
                };
                self.send(current_seq, resend_time)
            }
            SenderState::WaitingForSend {
                send_time,
                current_seq,
                timeout,
                exp_average_rtt,
            } => {
                assert_eq!(time, send_time);
                let next_send_time = self.next_send_time(time, exp_average_rtt);
                let resend_time = next_resend_time(time, timeout, next_send_time);
                self.state = SenderState::WaitingForAck {
                    first_send_time: Some(time),
                    next_send_time,
                    resend_time,
                    current_seq,
                    timeout,
                    exp_average_rtt,
                };
                self.send(current_seq, resend_time)
            }
        }
    }

    fn receive(&mut self, e: E, EffectContext { time, .. }: EffectContext) -> EffectResult<E> {
        let p = HasVariant::<Packet>::try_into(e).unwrap();
        match self.state {
            SenderState::Starting => {
                panic!()
            }
            SenderState::WaitingForAck {
                first_send_time,
                resend_time,
                mut current_seq,
                mut timeout,
                mut exp_average_rtt,
                next_send_time,
            } => {
                if p.seq != current_seq {
                    log!(self.logger, "Ignoring duplicate of packet {}", p.seq);
                    return ignore(resend_time);
                }
                if let Some(first_send_time) = first_send_time {
                    exp_average_rtt = new_exp_average(exp_average_rtt, time - first_send_time);
                    timeout = 2. * exp_average_rtt;
                    log!(
                        self.logger,
                        "Measured last sent time, so adjusted timeout to {}",
                        timeout
                    );
                }
                current_seq += 1;
                if time < next_send_time {
                    log!(self.logger, "Received ack, waiting before sending again...");
                    self.state = SenderState::WaitingForSend {
                        send_time: next_send_time,
                        current_seq,
                        timeout,
                        exp_average_rtt,
                    };
                    ignore(next_send_time)
                } else {
                    log!(self.logger, "Received ack, can send immediately.");
                    let next_send_time = self.next_send_time(time, exp_average_rtt);
                    let resend_time = next_resend_time(time, timeout, next_send_time);
                    self.state = SenderState::WaitingForAck {
                        first_send_time: Some(time),
                        resend_time,
                        current_seq,
                        timeout,
                        exp_average_rtt,
                        next_send_time,
                    };
                    self.send(current_seq, resend_time)
                }
            }
            SenderState::WaitingForSend { send_time, .. } => ignore(send_time),
        }
    }
}

pub struct Receiver<L> {
    destination: ComponentId,
    logger: L,
}

impl<'a, L> Receiver<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub const fn new<E>(destination: ComponentId, logger: L) -> Receiver<L>
    where
        E: HasVariant<Packet>,
    {
        Receiver {
            destination,
            logger,
        }
    }
}

impl<'a, E, L> Component<E> for Receiver<L>
where
    E: HasVariant<Packet>,
    L: Logger + 'a,
{
    fn tick(&mut self, _: EffectContext) -> EffectResult<E> {
        EffectResult {
            next_tick: None,
            effects: vec![],
        }
    }

    fn receive<'b>(
        &mut self,
        message: E,
        EffectContext { self_id, .. }: EffectContext,
    ) -> EffectResult<E> {
        let Packet {
            source,
            destination,
            seq,
        } = message.try_into().unwrap();
        assert_eq!(destination, self_id);
        log!(self.logger, "Bounced message {seq}");
        EffectResult {
            next_tick: None,
            effects: vec![Message::new(
                self.destination,
                Packet {
                    seq,
                    source: self_id,
                    destination: source,
                },
            )],
        }
    }
}
