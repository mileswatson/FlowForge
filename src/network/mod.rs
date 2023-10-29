use crate::{rand::Rng, simulation::Simulator};

pub mod config;

#[derive(Debug)]
pub struct Network {
    pub rtt: f32,
    pub throughput: f32,
    pub loss_rate: f32,
}

pub struct NetworkSimulator {
    sim: Simulator,
}

impl NetworkSimulator {
    #[must_use]
    pub fn new(network: &Network, rng: Rng) -> NetworkSimulator {
        NetworkSimulator {
            sim: Simulator::new(vec![], rng),
        }
    }
}
