use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    quantities::{bits_per_second, milliseconds, seconds, Information, InformationRate, TimeSpan},
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
    pub buffer_size: Option<DiscreteDistribution<Information>>,
    pub num_senders: DiscreteDistribution<u32>,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig {
            rtt: PositiveContinuousDistribution(ContinuousDistribution::Uniform {
                min: milliseconds(100.),
                max: milliseconds(200.),
            }),
            bandwidth: PositiveContinuousDistribution(ContinuousDistribution::Uniform {
                min: bits_per_second(10_000_000.),
                max: bits_per_second(20_000_000.),
            }),
            loss_rate: ProbabilityDistribution(ContinuousDistribution::Always { value: 0. }),
            buffer_size: None,
            num_senders: DiscreteDistribution::Uniform { min: 1, max: 16 },
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
            buffer_size: self.buffer_size.as_ref().map(|d| rng.sample(d)),
            num_senders: rng.sample(&self.num_senders) as usize,
            off_time: self.off_time.clone(),
            on_time: self.on_time.clone(),
        }
    }
}
