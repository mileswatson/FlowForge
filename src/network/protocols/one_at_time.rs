use crate::{
    logging::Logger,
    rand::Rng,
    simulation::{Component, EffectResult, HasVariant, Message, Time},
};

#[derive(Debug)]
pub struct Packet {
    seq: u64,
}

#[derive(Debug)]
pub struct Ack {
    seq: u64,
}

#[derive(Debug)]
pub struct Sender<L> {
    link: usize,
    current_seq: u64,
    next_timeout: Option<Time>,
    last_sent_time: Option<Time>,
    timeout: Time,
    exp_average_rtt: Time,
    logger: L,
}

impl<'a, L> Sender<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create<E>(link: usize, logger: L) -> Box<dyn Component<E> + 'a>
    where
        E: HasVariant<Packet> + HasVariant<Ack>,
    {
        Box::new(Sender {
            link,
            current_seq: 0,
            next_timeout: None,
            last_sent_time: None,
            timeout: 1.0,
            exp_average_rtt: 0.5,
            logger,
        })
    }

    fn send<E: HasVariant<Packet>>(&mut self, time: Time, resend: bool) -> EffectResult<E> {
        if resend {
            self.last_sent_time = None;
            log!(self.logger, "Resending {}", self.current_seq);
        } else {
            self.last_sent_time = Some(time);
            log!(self.logger, "Sending {}", self.current_seq);
        }
        self.next_timeout = Some(time + self.timeout);
        EffectResult {
            next_tick: self.next_timeout,
            effects: vec![Message::new(
                self.link,
                Packet {
                    seq: self.current_seq,
                },
            )],
        }
    }
}

impl<'a, E, L> Component<E> for Sender<L>
where
    E: HasVariant<Packet> + HasVariant<Ack>,
    L: Logger + 'a,
{
    fn tick(&mut self, time: Time, _rng: &mut Rng) -> EffectResult<E> {
        self.timeout *= 2.;
        log!(
            self.logger,
            "Timed out, so adjusted timeout to {}",
            self.timeout
        );
        self.send(time, true)
    }

    fn receive(&mut self, e: E, time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let p = HasVariant::<Ack>::try_into(e).unwrap();
        if p.seq != self.current_seq {
            log!(self.logger, "Ignoring duplicate of packet {}", p.seq);
            return EffectResult {
                next_tick: self.next_timeout,
                effects: vec![],
            };
        }
        log!(self.logger, "Received ack for {}", self.current_seq);
        if let Some(last_sent_time) = self.last_sent_time {
            const ALPHA: f64 = 0.8;
            self.exp_average_rtt =
                self.exp_average_rtt * ALPHA + (1. - ALPHA) * (time - last_sent_time);
            self.timeout = 2. * self.exp_average_rtt;
            log!(
                self.logger,
                "Measured last sent time, so adjusted timeout to {:?}",
                self.timeout
            );
        }
        self.current_seq += 1;
        self.send(time, false)
    }
}

pub struct Receiver<L> {
    destination: usize,
    logger: L,
}

impl<'a, L> Receiver<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create<E>(destination: usize, logger: L) -> Box<dyn Component<E> + 'a>
    where
        E: HasVariant<Packet> + HasVariant<Ack>,
    {
        Box::new(Receiver {
            destination,
            logger,
        })
    }
}

impl<'a, E, L> Component<E> for Receiver<L>
where
    E: HasVariant<Ack> + HasVariant<Packet>,
    L: Logger + 'a,
{
    fn tick(&mut self, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        EffectResult {
            next_tick: None,
            effects: vec![],
        }
    }

    fn receive(&mut self, message: E, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let Packet { seq } = message.try_into().unwrap();
        log!(self.logger, "Bounced message {seq}");
        EffectResult {
            next_tick: None,
            effects: vec![Message::new(self.destination, Ack { seq })],
        }
    }
}
