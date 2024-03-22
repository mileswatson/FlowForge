use std::ops::{Add, Mul};

use crate::quantities::{Float, Information, InformationRate, Time, TimeSpan};

use super::average::Average;

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
            total_time: TimeSpan::ZERO,
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
            total_time: TimeSpan::ZERO,
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
    enabled_time: TimeSpan,
) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
    assert!(!enabled_time.is_negative());
    if enabled_time == TimeSpan::ZERO {
        return Err(InfoRateMeterNeverEnabled);
    }
    #[allow(clippy::cast_precision_loss)]
    return Ok(total / enabled_time);
}

impl DisabledInfoRateMeter {
    #[must_use]
    pub const fn new() -> DisabledInfoRateMeter {
        DisabledInfoRateMeter {
            timer: DisabledTimer::new(),
            total: Information::ZERO,
        }
    }

    #[must_use]
    pub const fn enable(self, time: Time) -> EnabledInfoRateMeter {
        EnabledInfoRateMeter {
            timer: self.timer.enable(time),
            total: self.total,
        }
    }

    pub fn current_value(&self) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
        calculate_rate(self.total, self.timer.current_value())
    }
}

impl Default for DisabledInfoRateMeter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InfoRateMeterNeverEnabled;

impl EnabledInfoRateMeter {
    #[must_use]
    pub const fn new(time: Time) -> EnabledInfoRateMeter {
        EnabledInfoRateMeter {
            timer: EnabledTimer::new(time),
            total: Information::ZERO,
        }
    }
    pub fn record_info(&mut self, info: Information) {
        self.total = self.total + info;
    }

    pub fn current_value(&self, time: Time) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
        calculate_rate(self.total, self.timer.current_value(time))
    }

    #[must_use]
    pub fn disable(self, time: Time) -> DisabledInfoRateMeter {
        DisabledInfoRateMeter {
            timer: self.timer.disable(time),
            total: self.total,
        }
    }
}
