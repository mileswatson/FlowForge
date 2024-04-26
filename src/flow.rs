use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    quantities::{seconds, Float, InformationRate, TimeSpan},
    util::average::{Average, AverageIfSome, AveragePair, IterAverage, NoItems},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoPacketsAcked;

#[derive(Debug, Clone, PartialEq)]
pub struct FlowProperties {
    pub throughput: InformationRate,
    pub rtt: Result<TimeSpan, NoPacketsAcked>,
}

impl Display for FlowProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rtt {
            Ok(average_rtt) => write!(
                f,
                "FlowProperties {{ throughput: {}, rtt: {} }}",
                self.throughput, average_rtt
            ),
            Err(_) => write!(
                f,
                "FlowProperties {{ throughput: {}, rtt: NoPacketsAcked }}",
                self.throughput
            ),
        }
    }
}

impl Average for FlowProperties {
    type Aggregator =
        <AveragePair<InformationRate, AverageIfSome<TimeSpan>> as Average>::Aggregator;
    type Output = Result<FlowProperties, NoItems>;

    fn average(aggregator: Self::Aggregator) -> Self::Output {
        let (average_throughput, average_rtt) =
            AveragePair::<InformationRate, AverageIfSome<TimeSpan>>::average(aggregator);
        match average_throughput {
            Ok(average_throughput) => Ok(FlowProperties {
                throughput: average_throughput,
                rtt: average_rtt.map_err(|_| NoPacketsAcked),
            }),
            Err(NoItems) => {
                assert!(average_rtt.is_err());
                Err(NoItems)
            }
        }
    }

    fn new_aggregator() -> Self::Aggregator {
        AveragePair::<InformationRate, AverageIfSome<TimeSpan>>::new_aggregator()
    }

    fn aggregate(aggregator: Self::Aggregator, next: Self) -> Self::Aggregator {
        AveragePair::<InformationRate, AverageIfSome<TimeSpan>>::aggregate(
            aggregator,
            AveragePair(next.throughput, AverageIfSome::new(next.rtt.ok())),
        )
    }
}

#[derive(Debug)]
pub struct FlowNeverActive;

fn alpha_fairness(x: Float, alpha: Float) -> Float {
    let x = x + 0.000_001;
    if (alpha - 1.).abs() < 0.000_001 {
        x.ln()
    } else {
        x.powf(1. - alpha) / (1. - alpha)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NoActiveFlows;

pub trait UtilityFunction: Sync {
    fn utility(&self, flows: &[FlowProperties]) -> Result<Float, NoActiveFlows>;
}

#[derive(Serialize, Deserialize)]
pub enum UtilityConfig {
    AlphaFairness(AlphaFairness),
}

impl UtilityFunction for UtilityConfig {
    fn utility(&self, flows: &[FlowProperties]) -> Result<Float, NoActiveFlows> {
        match self {
            UtilityConfig::AlphaFairness(x) => x.utility(flows),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AlphaFairness {
    /// Fairness of throughput
    alpha: Float,
    /// Fairness of round-trip delay
    beta: Float,
    /// Relative importance of delay
    delta: Float,
    /// Worst-case (max) round-trip delay
    worst_case_rtt: TimeSpan,
}

impl AlphaFairness {
    pub const PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS: AlphaFairness = AlphaFairness {
        alpha: 1.,
        beta: 1.,
        delta: 1.,
        worst_case_rtt: seconds(10.),
    };

    pub const MINIMISE_FIXED_LENGTH_FILE_TRANSFER: AlphaFairness = AlphaFairness {
        alpha: 2.,
        beta: 0.,
        delta: 0.,
        worst_case_rtt: seconds(10.),
    };
}

impl UtilityFunction for AlphaFairness {
    fn utility(&self, flows: &[FlowProperties]) -> Result<Float, NoActiveFlows> {
        assert!(self.delta >= 0.);
        let flow_utility = |properties: &FlowProperties| {
            let throughput_utility = alpha_fairness(properties.throughput.value(), self.alpha);
            let rtt_utility = -self.delta
                * alpha_fairness(
                    properties
                        .rtt
                        .as_ref()
                        .unwrap_or(&self.worst_case_rtt)
                        .seconds()
                        .clamp(0., self.worst_case_rtt.seconds()),
                    self.beta,
                );
            throughput_utility + rtt_utility
                - (alpha_fairness(0., self.alpha)
                    - self.delta * alpha_fairness(self.worst_case_rtt.seconds(), self.beta))
        };
        flows
            .iter()
            .map(flow_utility)
            .average()
            .map_err(|_| NoActiveFlows)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        flow::FlowProperties,
        quantities::{bits_per_second, seconds, Float},
        util::average::{IterAverage, NoItems},
    };

    use super::NoPacketsAcked;

    #[test]
    fn flow_properties_average() {
        assert_eq!(
            vec![(0., None), (1., Some(3.))]
                .into_iter()
                .map(|(average_throughput, average_rtt)| FlowProperties {
                    throughput: bits_per_second(average_throughput),
                    rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                })
                .average(),
            Ok(FlowProperties {
                throughput: bits_per_second(0.5),
                rtt: Ok(seconds(3.))
            })
        );
        assert_eq!(
            vec![(0., None), (1., None)]
                .into_iter()
                .map(|(average_throughput, average_rtt)| FlowProperties {
                    throughput: bits_per_second(average_throughput),
                    rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                })
                .average(),
            Ok(FlowProperties {
                throughput: bits_per_second(0.5),
                rtt: Err(NoPacketsAcked)
            })
        );
        assert_eq!(
            vec![]
                .into_iter()
                .map(
                    |(average_throughput, average_rtt): (Float, Option<Float>)| FlowProperties {
                        throughput: bits_per_second(average_throughput),
                        rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                    }
                )
                .average(),
            Err(NoItems)
        );
    }
}
