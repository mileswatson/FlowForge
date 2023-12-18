use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    rand::{ContinuousDistribution, DiscreteDistribution},
    time::{Float, Rate, TimeSpan},
};

use super::Network;

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub rtt: ContinuousDistribution<Float>,
    pub packet_rate: ContinuousDistribution<Float>,
    pub loss_rate: ContinuousDistribution<Float>,
    pub buffer_size: Option<DiscreteDistribution<usize>>,
    pub num_senders: DiscreteDistribution<usize>,
    pub off_time: ContinuousDistribution<Float>,
    pub on_time: ContinuousDistribution<Float>,
}

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig {
            rtt: ContinuousDistribution::Normal {
                mean: 5e-3,
                std_dev: 1e-3,
            },
            packet_rate: ContinuousDistribution::Uniform { min: 12., max: 18. },
            loss_rate: ContinuousDistribution::Normal {
                mean: 0.1,
                std_dev: 0.01,
            },
            buffer_size: None,
            num_senders: DiscreteDistribution::Uniform { min: 1, max: 3 },
            off_time: ContinuousDistribution::Exponential { mean: 5. },
            on_time: ContinuousDistribution::Exponential { mean: 5. },
        }
    }
}

impl Distribution<Network> for NetworkConfig {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Network {
        Network {
            rtt: TimeSpan::new(rng.sample(&self.rtt)),
            packet_rate: Rate::new(rng.sample(&self.packet_rate)),
            loss_rate: rng.sample(&self.loss_rate),
            buffer_size: self.buffer_size.as_ref().map(|d| rng.sample(d)),
            num_senders: rng.sample(&self.num_senders),
            off_time: self.off_time.clone(),
            on_time: self.on_time.clone(),
        }
    }
}
