use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    quantities::{milliseconds, packets, packets_per_second, seconds, InformationRate, TimeSpan},
    rand::{
        ContinuousDistribution, DiscreteDistribution, PositiveContinuousDistribution,
        ProbabilityDistribution,
    },
};

use super::Network;

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub rtt: PositiveContinuousDistribution<TimeSpan>,
    pub bandwidth: PositiveContinuousDistribution<InformationRate>,
    pub loss_rate: ProbabilityDistribution,
    pub buffer_size: Option<DiscreteDistribution<u32>>,
    pub num_senders: DiscreteDistribution<u32>,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig {
            rtt: PositiveContinuousDistribution(ContinuousDistribution::Normal {
                mean: milliseconds(5.),
                std_dev: milliseconds(1.),
            }),
            bandwidth: PositiveContinuousDistribution(ContinuousDistribution::Uniform {
                min: packets_per_second(12.),
                max: packets_per_second(18.),
            }),
            loss_rate: ProbabilityDistribution(ContinuousDistribution::Normal {
                mean: 0.1,
                std_dev: 0.01,
            }),
            buffer_size: None,
            num_senders: DiscreteDistribution::Uniform { min: 1, max: 3 },
            off_time: PositiveContinuousDistribution(ContinuousDistribution::Exponential {
                mean: seconds(5.),
            }),
            on_time: PositiveContinuousDistribution(ContinuousDistribution::Exponential {
                mean: seconds(5.),
            }),
        }
    }
}

impl Distribution<Network> for NetworkConfig {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Network {
        Network {
            rtt: rng.sample(&self.rtt),
            packet_rate: rng.sample(&self.bandwidth),
            loss_rate: rng.sample(&self.loss_rate),
            buffer_size: self
                .buffer_size
                .as_ref()
                .map(|d| packets(u64::from(rng.sample(d)))),
            num_senders: rng.sample(&self.num_senders) as usize,
            off_time: self.off_time.clone(),
            on_time: self.on_time.clone(),
        }
    }
}
