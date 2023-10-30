pub mod config;
pub mod link;

#[derive(Debug)]
pub struct Network {
    pub rtt: f32,
    pub throughput: f32,
    pub loss_rate: f32,
}

#[cfg(test)]
mod tests {
    use crate::{
        rand::Rng,
        simulation::{Component, EffectResult, HasVariant, Message, Simulator, Time},
    };

    use super::link::Link;

    #[derive(Debug)]
    struct Packet {}

    #[derive(Debug)]
    enum Msg {
        Packet(Packet),
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
            }
        }
    }

    struct OneAtTimeSender {
        link: usize,
    }

    impl OneAtTimeSender {
        pub fn create<E>(link: usize) -> Box<dyn Component<E>>
        where
            E: HasVariant<Packet>,
        {
            Box::new(OneAtTimeSender { link })
        }
    }

    impl<E> Component<E> for OneAtTimeSender
    where
        E: HasVariant<Packet>,
    {
        fn tick(&mut self, time: Time, rng: &mut Rng) -> EffectResult<E> {
            self.receive(Packet {}.into(), time, rng)
        }

        fn receive(&mut self, _e: E, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
            dbg!("Sent!");
            EffectResult {
                next_tick: None,
                effects: vec![Message::new(self.link, Packet {})],
            }
        }
    }

    struct Bouncer {
        destination: usize,
    }

    impl Bouncer {
        pub fn create<E>(destination: usize) -> Box<dyn Component<E>>
        where
            E: HasVariant<Packet>,
        {
            Box::new(Bouncer { destination })
        }
    }

    impl<E> Component<E> for Bouncer
    where
        E: HasVariant<Packet>,
    {
        fn tick(&mut self, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
            EffectResult {
                next_tick: None,
                effects: vec![],
            }
        }

        fn receive(&mut self, message: E, _time: Time, _rng: &mut Rng) -> EffectResult<E> {
            EffectResult {
                next_tick: None,
                effects: vec![Message::new(self.destination, message)],
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
                        OneAtTimeSender::create(1),
                        Link::create(2, 0.1),
                        Bouncer::create(3),
                        Link::create(0, 0.1),
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
        sim.run_until(1.0);
    }
}
