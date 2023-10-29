use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::rand::ContinuousDistribution;

use super::Network;

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub rtt: ContinuousDistribution<f32>,
    pub throughput: ContinuousDistribution<f32>,
    pub loss_rate: ContinuousDistribution<f32>,
}

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig {
            rtt: ContinuousDistribution::Normal {
                mean: 5e-3,
                std_dev: 1e-3,
            },
            throughput: ContinuousDistribution::Uniform { min: 12., max: 18. },
            loss_rate: ContinuousDistribution::Normal {
                mean: 0.1,
                std_dev: 0.01,
            },
        }
    }
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
