use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::rand::ContinuousDistribution;

use super::Network;

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub rtt: ContinuousDistribution,
    pub throughput: ContinuousDistribution,
    pub loss_rate: ContinuousDistribution,
}

impl Distribution<Network> for NetworkConfig {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Network {
        Network {
            rtt: rng.sample(&self.rtt),
            throughput: rng.sample(&self.throughput),
            loss_rate: rng.sample(&self.loss_rate),
        }
    }
}
