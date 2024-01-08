use std::ops::{Add, Mul};

use rand_distr::num_traits::Zero;

use crate::{
    average::Average,
    time::{Float, Rate, Time, TimeSpan},
};

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
    total_time: TimeSpan,
}

#[derive(Clone, Debug)]
pub struct EnabledTimer {
    total_time: TimeSpan,
    current_start: Time,
}

impl DisabledTimer {
    #[must_use]
    pub const fn new() -> DisabledTimer {
        DisabledTimer {
            total_time: TimeSpan::new(0.),
        }
    }

    #[must_use]
    pub const fn enable(self, time: Time) -> EnabledTimer {
        EnabledTimer {
            total_time: self.total_time,
            current_start: time,
        }
    }

    #[must_use]
    pub const fn current_value(&self) -> TimeSpan {
        self.total_time
    }
}

impl EnabledTimer {
    #[must_use]
    pub const fn new(time: Time) -> EnabledTimer {
        EnabledTimer {
            total_time: TimeSpan::new(0.),
            current_start: time,
        }
    }

    #[must_use]
    pub fn disable(self, time: Time) -> DisabledTimer {
        DisabledTimer {
            total_time: self.total_time + (time - self.current_start),
        }
    }

    #[must_use]
    pub fn current_value(&self, time: Time) -> TimeSpan {
        self.total_time + (time - self.current_start)
    }
}

#[derive(Clone, Debug)]
pub struct DisabledRateMeter {
    timer: DisabledTimer,
    count: u64,
}

#[derive(Clone, Debug)]
pub struct EnabledRateMeter {
    timer: EnabledTimer,
    count: u64,
}

fn calculate_rate(count: u64, enabled_time: TimeSpan) -> Result<Rate, RateMeterNeverEnabled> {
    assert!(!enabled_time.is_negative());
    if enabled_time.is_zero() {
        return Err(RateMeterNeverEnabled);
    }
    #[allow(clippy::cast_precision_loss)]
    return Ok(count as f64 / enabled_time);
}

impl DisabledRateMeter {
    #[must_use]
    pub const fn new() -> DisabledRateMeter {
        DisabledRateMeter {
            timer: DisabledTimer::new(),
            count: 0,
        }
    }

    #[must_use]
    pub const fn enable(self, time: Time) -> EnabledRateMeter {
        EnabledRateMeter {
            timer: self.timer.enable(time),
            count: self.count,
        }
    }

    pub fn current_value(&self) -> Result<Rate, RateMeterNeverEnabled> {
        calculate_rate(self.count, self.timer.current_value())
    }
}

pub struct RateMeterNeverEnabled;

impl EnabledRateMeter {
    #[must_use]
    pub const fn new(time: Time) -> EnabledRateMeter {
        EnabledRateMeter {
            timer: EnabledTimer::new(time),
            count: 0,
        }
    }
    pub fn record_event(&mut self) {
        self.count += 1;
    }

    pub fn current_value(&self, time: Time) -> Result<Rate, RateMeterNeverEnabled> {
        calculate_rate(self.count, self.timer.current_value(time))
    }

    #[must_use]
    pub fn disable(self, time: Time) -> DisabledRateMeter {
        DisabledRateMeter {
            timer: self.timer.disable(time),
            count: self.count,
        }
    }
}
