use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use super::TimeSpan;

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct Time(TimeSpan);

impl Time {
    pub const MIN: Time = Time(TimeSpan::MIN);
    pub const MAX: Time = Time(TimeSpan::MAX);
    pub const SIM_START: Time = Time(TimeSpan::ZERO);

    #[must_use]
    pub fn from_sim_start(t: TimeSpan) -> Time {
        Time::SIM_START + t
    }
}

impl Sub<Time> for Time {
    type Output = TimeSpan;

    fn sub(self, other: Time) -> Self::Output {
        self.0 - other.0
    }
}

impl Add<TimeSpan> for Time {
    type Output = Time;

    fn add(self, other: TimeSpan) -> Self::Output {
        Time(self.0 + other)
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}t", self.0)
    }
}

#[must_use]
pub fn earliest(times: &[Time]) -> Time {
    times.iter().copied().min().unwrap_or(Time::MIN)
}

#[must_use]
pub fn earliest_opt(times: &[Option<Time>]) -> Option<Time> {
    times
        .iter()
        .fold(None, |prev, current| match (prev, *current) {
            (Some(t1), Some(t2)) => Some(Time::min(t1, t2)),
            (m, None) | (None, m) => m,
        })
}

#[must_use]
pub fn latest(times: &[Time]) -> Time {
    times.iter().copied().max().unwrap_or(Time::MAX)
}

#[must_use]
pub fn latest_opt(times: &[Option<Time>]) -> Option<Time> {
    times
        .iter()
        .fold(None, |prev, current| match (prev, *current) {
            (Some(t1), Some(t2)) => Some(Time::max(t1, t2)),
            (m, None) | (None, m) => m,
        })
}
