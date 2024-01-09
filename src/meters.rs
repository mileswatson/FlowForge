use std::ops::{Add, Mul};

use rand_distr::num_traits::Zero;
use uom::{
    si::{
        f64::{self, InformationRate, Time},
        information::bit,
        u64::Information,
    },
    ConstZero,
};

use crate::{average::Average, time::TimePoint, Float};

#[derive(Clone, Debug)]
pub struct Mean<T>
where
    T: Average,
{
    aggregator: T::Aggregator,
}

impl<T> Mean<T>
where
    T: Average,
    T::Aggregator: Clone,
{
    #[must_use]
    pub fn new() -> Mean<T> {
        Mean::default()
    }

    pub fn record(&mut self, value: T) {
        self.aggregator = T::aggregate(self.aggregator.clone(), value);
    }

    #[must_use]
    pub fn value(&self) -> T::Output {
        T::average(self.aggregator.clone())
    }
}

impl<T> Default for Mean<T>
where
    T: Average,
{
    fn default() -> Self {
        Mean {
            aggregator: T::new_aggregator(),
        }
    }
}

#[derive(Debug)]
pub struct EWMA<T> {
    update_weight: Float,
    current: Option<T>,
}

impl<T> EWMA<T>
where
    T: Add<T, Output = T> + Copy,
    Float: Mul<T, Output = T>,
{
    #[must_use]
    pub const fn new(update_weight: Float) -> EWMA<T> {
        EWMA {
            update_weight,
            current: None,
        }
    }

    pub fn update(&mut self, value: T) -> T {
        let new_value = match self.current {
            Some(current) => (1. - self.update_weight) * current + self.update_weight * value,
            None => value,
        };
        self.current = Some(new_value);
        new_value
    }

    pub const fn value(&self) -> Option<T> {
        self.current
    }
}

#[derive(Clone, Debug)]
pub struct DisabledTimer {
    total_time: Time,
}

#[derive(Clone, Debug)]
pub struct EnabledTimer {
    total_time: Time,
    current_start: TimePoint,
}

impl DisabledTimer {
    #[must_use]
    pub const fn new() -> DisabledTimer {
        DisabledTimer {
            total_time: Time::ZERO,
        }
    }

    #[must_use]
    pub const fn enable(self, timepoint: TimePoint) -> EnabledTimer {
        EnabledTimer {
            total_time: self.total_time,
            current_start: timepoint,
        }
    }

    #[must_use]
    pub const fn current_value(&self) -> Time {
        self.total_time
    }
}

impl Default for DisabledTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl EnabledTimer {
    #[must_use]
    pub const fn new(timepoint: TimePoint) -> EnabledTimer {
        EnabledTimer {
            total_time: Time::ZERO,
            current_start: timepoint,
        }
    }

    #[must_use]
    pub fn disable(self, timepoint: TimePoint) -> DisabledTimer {
        DisabledTimer {
            total_time: self.total_time + (timepoint - self.current_start),
        }
    }

    #[must_use]
    pub fn current_value(&self, timepoint: TimePoint) -> Time {
        self.total_time + (timepoint - self.current_start)
    }
}

#[derive(Clone, Debug)]
pub struct DisabledInfoRateMeter {
    timer: DisabledTimer,
    total: Information,
}

#[derive(Clone, Debug)]
pub struct EnabledInfoRateMeter {
    timer: EnabledTimer,
    total: Information,
}

fn calculate_rate(
    total: Information,
    enabled_time: Time,
) -> Result<InformationRate, RateMeterNeverEnabled> {
    assert!(!enabled_time.is_sign_negative());
    if enabled_time.is_zero() {
        return Err(RateMeterNeverEnabled);
    }
    #[allow(clippy::cast_precision_loss)]
    Ok((f64::Information::new::<bit>(total.get::<bit>() as f64) / enabled_time).into())
}

impl DisabledInfoRateMeter {
    #[must_use]
    pub fn new() -> DisabledInfoRateMeter {
        DisabledInfoRateMeter {
            timer: DisabledTimer::new(),
            total: Information::zero(),
        }
    }

    #[must_use]
    pub const fn enable(self, timepoint: TimePoint) -> EnabledInfoRateMeter {
        EnabledInfoRateMeter {
            timer: self.timer.enable(timepoint),
            total: self.total,
        }
    }

    pub fn current_value(&self) -> Result<InformationRate, RateMeterNeverEnabled> {
        calculate_rate(self.total, self.timer.current_value())
    }
}

impl Default for DisabledInfoRateMeter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RateMeterNeverEnabled;

impl EnabledInfoRateMeter {
    #[must_use]
    pub fn new(timepoint: TimePoint) -> EnabledInfoRateMeter {
        EnabledInfoRateMeter {
            timer: EnabledTimer::new(timepoint),
            total: Information::zero(),
        }
    }
    pub fn record_info(&mut self, info: Information) {
        self.total.get::<bit>();
        self.total += info;
    }

    pub fn current_value(
        &self,
        timepoint: TimePoint,
    ) -> Result<InformationRate, RateMeterNeverEnabled> {
        calculate_rate(self.total, self.timer.current_value(timepoint))
    }

    #[must_use]
    pub fn disable(self, timepoint: TimePoint) -> DisabledInfoRateMeter {
        DisabledInfoRateMeter {
            timer: self.timer.disable(timepoint),
            total: self.total,
        }
    }
}
