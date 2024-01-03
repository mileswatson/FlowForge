use std::{cell::RefCell, rc::Rc};

use ordered_float::NotNan;
use rand_distr::num_traits::Zero;
use serde::{Deserialize, Serialize};

use crate::{
    average::{Average, AverageIfSome, AverageSeparately, AverageTogether, IterAverage, NoItems},
    time::{Float, Rate, Time, TimeSpan},
};

#[derive(Clone, Debug)]
pub struct NoPacketsAcked;

#[derive(Debug, Clone)]
pub struct FlowProperties {
    pub average_throughput: Rate,
    pub average_rtt: Result<TimeSpan, NoPacketsAcked>,
}

impl Average for FlowProperties {
    type Output = Result<FlowProperties, NoItems>;

    fn average<I>(items: I) -> Self::Output
    where
        I: IntoIterator<Item = Self>,
    {
        let (average_throughput, average_rtt) = items
            .into_iter()
            .map(|props| {
                AverageSeparately(
                    props.average_throughput,
                    AverageIfSome::new(props.average_rtt.ok()),
                )
            })
            .average();
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
}

#[derive(Debug)]
pub struct FlowNeverActive {}

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
    if (1. - alpha).is_zero() {
        x.ln()
    } else {
        x.powf(1. - alpha) / (1. - alpha)
    }
}

#[derive(Debug)]
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
        #[allow(clippy::cast_precision_loss)]
        match self {
            FlowUtilityAggregator::Mean => scores
                .map(AverageTogether::new)
                .average()
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
    /// Aggregation
    flow_utility_aggregator: FlowUtilityAggregator,
}

impl AlphaFairness {
    pub const PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS: AlphaFairness = AlphaFairness {
        alpha: 1.,
        beta: 1.,
        delta: 1.,
        flow_utility_aggregator: FlowUtilityAggregator::Mean,
    };

    pub const MINIMISE_FIXED_LENGTH_FILE_TRANSFER: AlphaFairness = AlphaFairness {
        alpha: 2.,
        beta: 0.,
        delta: 0.,
        flow_utility_aggregator: FlowUtilityAggregator::Mean,
    };

    fn flow_utility(&self, properties: &FlowProperties) -> Float {
        let throughput_utility = alpha_fairness(properties.average_throughput.value(), self.alpha);
        let rtt_utility = match properties.average_rtt {
            Ok(average_rtt) => -self.delta * alpha_fairness(average_rtt.value(), self.beta),
            Err(NoPacketsAcked) => 0.,
        };
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
