use std::{cell::RefCell, rc::Rc};

use ordered_float::NotNan;
use rand_distr::num_traits::Zero;
use serde::{Deserialize, Serialize};

use crate::time::{Float, Rate, Time, TimeSpan};

pub struct NoPacketsAcked;

pub struct FlowProperties {
    pub average_throughput: Rate,
    pub average_rtt: Result<TimeSpan, NoPacketsAcked>,
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
    fn total_utility(&self, flows: &[Rc<dyn Flow>], time: Time) -> Result<Float, NoActiveFlows>;
}

#[derive(Serialize, Deserialize)]
pub enum FlowUtilityAggregator {
    Mean,
    Minimum,
}

impl FlowUtilityAggregator {
    pub fn total_utility<F>(
        &self,
        flows: &[Rc<dyn Flow>],
        flow_utility: F,
    ) -> Result<Float, NoActiveFlows>
    where
        F: Fn(&dyn Flow) -> Result<Float, FlowNeverActive>,
    {
        let scores: Vec<_> = flows
            .iter()
            .filter_map(|flow| flow_utility(&**flow).ok())
            .collect();
        #[allow(clippy::cast_precision_loss)]
        match self {
            FlowUtilityAggregator::Mean => {
                if scores.is_empty() {
                    Err(NoActiveFlows)
                } else {
                    Ok(scores.iter().sum::<Float>() / scores.len() as Float)
                }
            }
            FlowUtilityAggregator::Minimum => scores
                .into_iter()
                .map(NotNan::new)
                .map(Result::unwrap)
                .min()
                .map(NotNan::into_inner)
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

    fn flow_utility(&self, flow: &dyn Flow, time: Time) -> Result<Float, FlowNeverActive> {
        flow.properties(time).map(|properties| {
            let throughput_utility =
                alpha_fairness(properties.average_throughput.value(), self.alpha);
            let rtt_utility = match properties.average_rtt {
                Ok(average_rtt) => -self.delta * alpha_fairness(average_rtt.value(), self.beta),
                Err(NoPacketsAcked) => 0.,
            };
            throughput_utility + rtt_utility
        })
    }
}

impl UtilityFunction for AlphaFairness {
    fn total_utility(&self, flows: &[Rc<dyn Flow>], time: Time) -> Result<Float, NoActiveFlows> {
        self.flow_utility_aggregator
            .total_utility(flows, |flow| self.flow_utility(flow, time))
    }
}
