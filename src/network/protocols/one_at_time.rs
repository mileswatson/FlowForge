use crate::{
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
pub struct Sender {
    link: usize,
    current_seq: u64,
    next_timeout: Option<Time>,
    last_sent_time: Option<Time>,
    timeout: Time,
    exp_average_rtt: Time,
}

impl Sender {
    #[must_use]
    pub fn create<E>(link: usize) -> Box<dyn Component<E>>
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
        })
    }

    fn send<E: HasVariant<Packet>>(&mut self, time: Time, resend: bool) -> EffectResult<E> {
        if resend {
            self.last_sent_time = None;
            println!("Resending {}", self.current_seq);
        } else {
            self.last_sent_time = Some(time);
            println!("Sending {}", self.current_seq);
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

impl<E> Component<E> for Sender
where
    E: HasVariant<Packet> + HasVariant<Ack>,
{
    fn tick(&mut self, time: Time, _rng: &mut Rng) -> EffectResult<E> {
        self.timeout *= 2.;
        println!("Timed out, so adjusted timeout to {}", self.timeout);
        self.send(time, true)
    }

    fn receive(&mut self, e: E, time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let p = HasVariant::<Ack>::try_into(e).unwrap();
        if p.seq != self.current_seq {
            println!("Ignoring duplicate of packet {}", p.seq);
            return EffectResult {
                next_tick: self.next_timeout,
                effects: vec![],
            };
        }
        println!("Received ack for {}", self.current_seq);
        if let Some(last_sent_time) = self.last_sent_time {
            const ALPHA: f64 = 0.8;
            self.exp_average_rtt =
                self.exp_average_rtt * ALPHA + (1. - ALPHA) * (time - last_sent_time);
            self.timeout = 2. * self.exp_average_rtt;
            println!(
                "Measured last sent time, so adjusted timeout to {:?}",
                self.timeout
            );
        }
        self.current_seq += 1;
        self.send(time, false)
    }
}

pub struct Receiver {
    destination: usize,
}

impl Receiver {
    #[must_use]
    pub fn create<E>(destination: usize) -> Box<dyn Component<E>>
    where
        E: HasVariant<Packet> + HasVariant<Ack>,
    {
        Box::new(Receiver { destination })
    }
}

impl<E> Component<E> for Receiver
where
    E: HasVariant<Ack> + HasVariant<Packet>,
{
    fn tick(&mut self, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        EffectResult {
            next_tick: None,
            effects: vec![],
        }
    }

    fn receive(&mut self, message: E, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
        let Packet { seq } = message.try_into().unwrap();
        EffectResult {
            next_tick: None,
            effects: vec![Message::new(self.destination, Ack { seq })],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::link::Link,
        rand::Rng,
        simulation::{HasVariant, Simulator, Time},
    };

    use super::{Ack, Packet, Receiver, Sender};

    #[derive(Debug)]
    enum Msg {
        Packet(Packet),
        Ack(Ack),
    }

    impl From<Packet> for Msg {
        fn from(value: Packet) -> Self {
            Msg::Packet(value)
        }
    }

    impl HasVariant<Packet> for Msg {
        fn try_into(self) -> Result<Packet, Self> {
            match self {
                Msg::Packet(p) => Ok(p),
                Msg::Ack(_) => Err(self),
            }
        }
    }

    impl From<Ack> for Msg {
        fn from(ack: Ack) -> Self {
            Msg::Ack(ack)
        }
    }

    impl HasVariant<Ack> for Msg {
        fn try_into(self) -> Result<Ack, Self> {
            match self {
                Msg::Ack(ack) => Ok(ack),
                Msg::Packet(_) => Err(self),
            }
        }
    }

    struct NetworkSimulator {
        sim: Simulator<Msg>,
    }

    impl NetworkSimulator {
        #[must_use]
        pub fn new(rng: Rng) -> NetworkSimulator {
            NetworkSimulator {
                sim: Simulator::new(
                    vec![
                        Sender::create(1),
                        Link::create(2, 1.5),
                        Receiver::create(3),
                        Link::create(0, 1.5),
                    ],
                    rng,
                ),
            }
        }

        pub fn run_until(self, time: Time) {
            self.sim.run_until(time);
        }
    }

    #[test]
    fn mock_network() {
        let sim = NetworkSimulator::new(Rng::from_seed(1_234_987_348));
        sim.run_until(100.);
    }
}
