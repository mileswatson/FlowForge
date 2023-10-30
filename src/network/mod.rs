use crate::{
    rand::Rng,
    simulation::{Component, EffectResult, Time},
};

pub mod config;
pub mod link;

#[derive(Debug)]
pub struct Network {
    pub rtt: f32,
    pub throughput: f32,
    pub loss_rate: f32,
}

trait CanReceive<T>: Component {
    fn receive(&mut self, time: Time, message: T, rng: &mut Rng) -> EffectResult;
}

#[cfg(test)]
mod tests {
    use std::{any::Any, ops::DerefMut};

    use crate::{
        rand::Rng,
        simulation::{Component, Effect, EffectResult, Simulator, Time},
    };

    use super::{link::Link, CanReceive, Network};

    struct Sender {
        link: usize,
    }

    impl Sender {
        pub fn create(link: usize) -> Box<dyn Component> {
            Box::new(Sender { link })
        }
    }

    impl Component for Sender {
        fn tick(&mut self, time: Time, rng: &mut Rng) -> EffectResult {
            self.receive(time, (), rng)
        }

        fn as_any(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl CanReceive<()> for Sender {
        fn receive(&mut self, time: Time, message: (), rng: &mut Rng) -> EffectResult {
            EffectResult {
                next_tick: None,
                effects: vec![Effect::new::<dyn CanReceive<()>>(
                    self.link,
                    |c, time, rng| c.receive(time, (), rng),
                )],
            }
        }
    }

    struct Bouncer {
        destination: usize,
    }

    impl Bouncer {
        pub fn create(destination: usize) -> Box<dyn Component> {
            Box::new(Bouncer { destination })
        }
    }

    impl Component for Bouncer {
        fn tick(&mut self, time: Time, rng: &mut Rng) -> EffectResult {
            EffectResult {
                next_tick: None,
                effects: vec![],
            }
        }

        fn as_any(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl<P: 'static> CanReceive<P> for Bouncer {
        fn receive(&mut self, time: Time, message: P, rng: &mut Rng) -> EffectResult {
            EffectResult {
                next_tick: None,
                effects: vec![Effect::new(
                    self.destination,
                    |c: &mut dyn CanReceive<P>, time, rng| c.receive(time, message, rng),
                )],
            }
        }
    }

    struct NetworkSimulator {
        sim: Simulator,
    }

    impl NetworkSimulator {
        #[must_use]
        pub fn new(rng: Rng) -> NetworkSimulator {
            NetworkSimulator {
                sim: Simulator::new(
                    vec![
                        Sender::create(1),
                        Link::<()>::create(2, 0.1),
                        //Bouncer::create(3),
                        //Link::<()>::create(0, 0.1),
                    ],
                    rng,
                ),
            }
        }

        pub fn run(self) {
            self.sim.run();
        }
    }

    #[test]
    fn mock_network() {
        let sim = NetworkSimulator::new(Rng::from_seed(1_234_987_348));
        sim.run();
    }
}
