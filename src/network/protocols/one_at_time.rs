use crate::{
    logging::Logger,
    network::link::Routable,
    simulation::{Component, EffectContext, EffectResult, HasVariant, Message, Time, TimeSpan},
};

#[derive(Debug)]
pub struct Packet {
    seq: u64,
    source: usize,
    destination: usize,
}

impl Routable for Packet {
    fn pop_next_hop(&mut self) -> usize {
        self.destination
    }
}

#[derive(Debug)]
pub struct Sender<L> {
    index: usize,
    link: usize,
    destination: usize,
    current_seq: u64,
    next_timeout: Option<Time>,
    last_sent_time: Option<Time>,
    timeout: TimeSpan,
    exp_average_rtt: TimeSpan,
    logger: L,
}

impl<'a, L> Sender<L>
where
    L: Logger + 'a,
{
    #[must_use]
    pub fn create<E>(
        index: usize,
        link: usize,
        destination: usize,
        logger: L,
    ) -> Box<dyn Component<E> + 'a>
    where
        E: HasVariant<Packet>,
    {
        Box::new(Sender {
            index,
            link,
            destination,
            current_seq: 0,
            next_timeout: None,
            last_sent_time: None,
            timeout: TimeSpan::new(1.0),
            exp_average_rtt: TimeSpan::new(0.5),
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
                    source: self.index,
                    destination: self.destination,
                },
            )],
        }
    }
}

impl<'a, E, L> Component<E> for Sender<L>
where
    E: HasVariant<Packet>,
    L: Logger + 'a,
{
    fn tick(&mut self, EffectContext { time, .. }: EffectContext) -> EffectResult<E> {
        self.timeout *= 2.;
        log!(
            self.logger,
            "Timed out, so adjusted timeout to {}",
            self.timeout
        );
        self.send(time, true)
    }

    fn receive(&mut self, e: E, EffectContext { time, .. }: EffectContext) -> EffectResult<E> {
        let p = HasVariant::<Packet>::try_into(e).unwrap();
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
                ALPHA * self.exp_average_rtt + (1. - ALPHA) * (time - last_sent_time);
            self.timeout = 2. * self.exp_average_rtt;
            log!(
                self.logger,
                "Measured last sent time, so adjusted timeout to {}",
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
        E: HasVariant<Packet>,
    {
        Box::new(Receiver {
            destination,
            logger,
        })
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
        EffectContext { self_index, .. }: EffectContext,
    ) -> EffectResult<E> {
        let Packet {
            source,
            destination,
            seq,
        } = message.try_into().unwrap();
        assert_eq!(destination, self_index);
        log!(self.logger, "Bounced message {seq}");
        EffectResult {
            next_tick: None,
            effects: vec![Message::new(
                self.destination,
                Packet {
                    seq,
                    source: self_index,
                    destination: source,
                },
            )],
        }
    }
}
