use std::{cell::RefCell, iter::once, rc::Rc};

use ordered_float::NotNan;
use rand_distr::num_traits::Zero;
use serde::{Deserialize, Serialize};

use crate::{
    average::{average, Average, AveragePair},
    time::{Float, Rate, Time, TimeSpan},
};

#[derive(Clone)]
pub struct NoPacketsAcked;

#[derive(Clone)]
pub struct FlowProperties {
    pub average_throughput: Rate,
    pub average_rtt: Result<TimeSpan, NoPacketsAcked>,
}

impl Average for FlowProperties {
    fn average<I>(first_item: Self, remaining_items: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        let remaining_items: Vec<_> = remaining_items.into_iter().collect();
        let average_throughput = Average::average(
            first_item.average_throughput,
            remaining_items.iter().map(|x| x.average_throughput),
        );
        let average_rtt = average(
            once(first_item)
                .chain(remaining_items)
                .filter_map(|x| x.average_rtt.ok()),
        )
        .map_err(|_| NoPacketsAcked);
        FlowProperties {
            average_throughput,
            average_rtt,
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
            FlowUtilityAggregator::Mean => average(scores.map(AveragePair::new))
                .map(AveragePair::into_inner)
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
