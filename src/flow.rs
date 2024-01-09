use std::{cell::RefCell, rc::Rc};

use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    average::{Average, AverageIfSome, AveragePair, IterAverage, NoItems, SameEmptiness},
    quantities::{seconds, Float, InformationRate, Time, TimeSpan},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoPacketsAcked;

#[derive(Debug, Clone, PartialEq)]
pub struct FlowProperties {
    pub average_throughput: InformationRate,
    pub average_rtt: Result<TimeSpan, NoPacketsAcked>,
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

pub trait Flow {
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive>;
}

impl<T> Flow for RefCell<T>
where
    T: Flow,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.borrow().properties(current_time)
    }
}

impl<T> Flow for Rc<T>
where
    T: Flow,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.as_ref().properties(current_time)
    }
}

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

#[derive(Serialize, Deserialize)]
pub enum UtilityConfig {
    AlphaFairness(AlphaFairness),
}

impl UtilityConfig {
    #[must_use]
    pub fn inner(&self) -> &dyn UtilityFunction {
        match self {
            UtilityConfig::AlphaFairness(x) => x,
        }
    }
}

pub trait UtilityFunction: Sync {
    /// Calculates flow properties and the total utility of a network simulation.
    fn total_utility<'a>(
        &self,
        flows: &[Rc<dyn Flow + 'a>],
        time: Time,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>;
}

#[derive(Serialize, Deserialize)]
pub enum FlowUtilityAggregator {
    Mean,
    Minimum,
}

impl FlowUtilityAggregator {
    pub fn total_utility<'a, F>(
        &self,
        flows: &[Rc<dyn Flow + 'a>],
        flow_utility: F,
        time: Time,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>
    where
        F: Fn(&FlowProperties) -> Float,
    {
        let scores = flows
            .iter()
            .filter_map(|flow| flow.properties(time).map(|x| (flow_utility(&x), x)).ok());
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

impl UtilityFunction for AlphaFairness {
    fn total_utility<'a>(
        &self,
        flows: &[Rc<dyn Flow + 'a>],
        time: Time,
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        self.flow_utility_aggregator
            .total_utility(flows, |flow| self.flow_utility(flow), time)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use itertools::Itertools;

    use crate::{
        average::{IterAverage, NoItems},
        quantities::{bits_per_second, seconds, Float, Time},
    };

    use super::{Flow, FlowNeverActive, FlowProperties, FlowUtilityAggregator, NoPacketsAcked};

    impl Flow for Option<FlowProperties> {
        fn properties(&self, _current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
            self.clone().ok_or(FlowNeverActive)
        }
    }

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
            .map(|x| {
                Rc::new(Some(FlowProperties {
                    average_throughput: bits_per_second(Float::from(x)),
                    average_rtt: Err(NoPacketsAcked),
                })) as Rc<dyn Flow>
            })
            .collect_vec();
        assert_eq!(
            FlowUtilityAggregator::Mean.total_utility(
                &flows,
                |props| props.average_throughput.value(),
                Time::SIM_START,
            ),
            Ok((
                2.,
                FlowProperties {
                    average_throughput: bits_per_second(2.),
                    average_rtt: Err(NoPacketsAcked)
                }
            ))
        );
        assert_eq!(
            FlowUtilityAggregator::Minimum.total_utility(
                &flows,
                |props| props.average_throughput.value(),
                Time::SIM_START,
            ),
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
