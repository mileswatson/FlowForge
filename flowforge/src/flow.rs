use std::fmt::Display;

use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    core::average::{Average, AverageIfSome, AveragePair, IterAverage, NoItems, SameEmptiness},
    quantities::{seconds, Float, InformationRate, TimeSpan},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoPacketsAcked;

#[derive(Debug, Clone, PartialEq)]
pub struct FlowProperties {
    pub average_throughput: InformationRate,
    pub average_rtt: Result<TimeSpan, NoPacketsAcked>,
}

impl Display for FlowProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.average_rtt {
            Ok(average_rtt) => write!(
                f,
                "FlowProperties {{ throughput: {}, rtt: {} }}",
                self.average_throughput, average_rtt
            ),
            Err(_) => write!(
                f,
                "FlowProperties {{ throughput: {}, rtt: NoPacketsAcked }}",
                self.average_throughput
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
                average_throughput,
                average_rtt: average_rtt.map_err(|_| NoPacketsAcked),
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
            AveragePair(
                next.average_throughput,
                AverageIfSome::new(next.average_rtt.ok()),
            ),
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
    /// Calculates flow properties and the total utility of a network simulation.
    fn total_utility(
        &self,
        flows: &[FlowProperties],
    ) -> Result<(Float, FlowProperties), NoActiveFlows>;

    fn flow_utility(&self, flow: &FlowProperties) -> Float;
}

#[derive(Serialize, Deserialize)]
pub enum UtilityConfig {
    AlphaFairness(AlphaFairness),
}

impl UtilityFunction for UtilityConfig {
    fn total_utility(
        &self,
        flows: &[FlowProperties],
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        match self {
            UtilityConfig::AlphaFairness(x) => x.total_utility(flows),
        }
    }

    fn flow_utility(&self, flow: &FlowProperties) -> Float {
        match self {
            UtilityConfig::AlphaFairness(x) => x.flow_utility(flow),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum FlowUtilityAggregator {
    Mean,
    Minimum,
}

impl FlowUtilityAggregator {
    pub fn total_utility<F>(
        &self,
        flows: &[FlowProperties],
        flow_utility: F,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>
    where
        F: Fn(&FlowProperties) -> Float,
    {
        let scores = flows.iter().map(|flow| (flow_utility(flow), flow.clone()));
        match self {
            FlowUtilityAggregator::Mean => scores
                .map(AveragePair::new)
                .average()
                .assert_same_emptiness()
                .map_err(|_| NoActiveFlows),
            FlowUtilityAggregator::Minimum => scores
                .map(|(score, properties)| (NotNan::new(score).unwrap(), properties))
                .min_by_key(|(score, _)| *score)
                .map(|(score, properties)| (score.into_inner(), properties))
                .ok_or(NoActiveFlows),
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
    /// Aggregation
    flow_utility_aggregator: FlowUtilityAggregator,
}

impl AlphaFairness {
    pub const PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS: AlphaFairness = AlphaFairness {
        alpha: 1.,
        beta: 1.,
        delta: 1.,
        worst_case_rtt: seconds(10.),
        flow_utility_aggregator: FlowUtilityAggregator::Mean,
    };

    pub const MINIMISE_FIXED_LENGTH_FILE_TRANSFER: AlphaFairness = AlphaFairness {
        alpha: 2.,
        beta: 0.,
        delta: 0.,
        worst_case_rtt: seconds(10.),
        flow_utility_aggregator: FlowUtilityAggregator::Mean,
    };
}

impl UtilityFunction for AlphaFairness {
    fn total_utility(
        &self,
        flows: &[FlowProperties],
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        self.flow_utility_aggregator
            .total_utility(flows, |flow| self.flow_utility(flow))
    }

    fn flow_utility(&self, properties: &FlowProperties) -> Float {
        let throughput_utility = alpha_fairness(properties.average_throughput.value(), self.alpha);
        let rtt_utility = -self.delta
            * alpha_fairness(
                properties
                    .average_rtt
                    .as_ref()
                    .unwrap_or(&self.worst_case_rtt)
                    .seconds()
                    .clamp(0., self.worst_case_rtt.seconds()),
                self.beta,
            );
        throughput_utility + rtt_utility
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{
        core::average::{IterAverage, NoItems},
        flow::FlowProperties,
        quantities::{bits_per_second, seconds, Float},
    };

    use super::{FlowUtilityAggregator, NoPacketsAcked};

    #[test]
    fn flow_properties_average() {
        assert_eq!(
            vec![(0., None), (1., Some(3.))]
                .into_iter()
                .map(|(average_throughput, average_rtt)| FlowProperties {
                    average_throughput: bits_per_second(average_throughput),
                    average_rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                })
                .average(),
            Ok(FlowProperties {
                average_throughput: bits_per_second(0.5),
                average_rtt: Ok(seconds(3.))
            })
        );
        assert_eq!(
            vec![(0., None), (1., None)]
                .into_iter()
                .map(|(average_throughput, average_rtt)| FlowProperties {
                    average_throughput: bits_per_second(average_throughput),
                    average_rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                })
                .average(),
            Ok(FlowProperties {
                average_throughput: bits_per_second(0.5),
                average_rtt: Err(NoPacketsAcked)
            })
        );
        assert_eq!(
            vec![]
                .into_iter()
                .map(
                    |(average_throughput, average_rtt): (Float, Option<Float>)| FlowProperties {
                        average_throughput: bits_per_second(average_throughput),
                        average_rtt: average_rtt.map(seconds).ok_or(NoPacketsAcked),
                    }
                )
                .average(),
            Err(NoItems)
        );
    }

    #[test]
    fn flow_utility_aggregator() {
        let flows = (0..5)
            .map(|x| FlowProperties {
                average_throughput: bits_per_second(Float::from(x)),
                average_rtt: Err(NoPacketsAcked),
            })
            .collect_vec();
        assert_eq!(
            FlowUtilityAggregator::Mean
                .total_utility(&flows, |props| props.average_throughput.value(),),
            Ok((
                2.,
                FlowProperties {
                    average_throughput: bits_per_second(2.),
                    average_rtt: Err(NoPacketsAcked)
                }
            ))
        );
        assert_eq!(
            FlowUtilityAggregator::Minimum
                .total_utility(&flows, |props| props.average_throughput.value(),),
            Ok((
                0.,
                FlowProperties {
                    average_throughput: bits_per_second(0.),
                    average_rtt: Err(NoPacketsAcked)
                }
            ))
        );
    }
}
