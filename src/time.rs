use std::{
    fmt::Debug,
    fmt::Display,
    ops::{Add, Div, Mul, MulAssign, Sub},
};

use format_num::format_num;
use rand_distr::num_traits::Zero;
use serde::{Deserialize, Serialize};

pub type Float = f64;

#[derive(PartialEq, PartialOrd, Clone, Copy, Serialize, Deserialize)]
pub struct TimeSpan(Float);

impl Debug for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

impl TimeSpan {
    #[must_use]
    pub const fn new(ts: Float) -> TimeSpan {
        TimeSpan(ts)
    }

    #[must_use]
    pub const fn value(&self) -> Float {
        self.0
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0 < 0.
    }
}

impl Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", format_num!(".3s", self.0))
    }
}

impl From<i32> for TimeSpan {
    fn from(value: i32) -> Self {
        TimeSpan::new(value.into())
    }
}

impl From<Float> for TimeSpan {
    fn from(value: Float) -> Self {
        TimeSpan::new(value)
    }
}

impl Zero for TimeSpan {
    fn zero() -> Self {
        TimeSpan::new(0.)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0.
    }
}

impl Add for TimeSpan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TimeSpan::new(self.0 + rhs.0)
    }
}

impl Mul<TimeSpan> for Float {
    type Output = TimeSpan;

    fn mul(self, rhs: TimeSpan) -> Self::Output {
        TimeSpan::new(self * rhs.0)
    }
}

impl MulAssign<Float> for TimeSpan {
    fn mul_assign(&mut self, rhs: Float) {
        self.0 *= rhs;
    }
}

impl Div<Float> for TimeSpan {
    type Output = TimeSpan;

    fn div(self, rhs: Float) -> Self::Output {
        TimeSpan::new(self.0 / rhs)
    }
}

impl Div<TimeSpan> for TimeSpan {
    type Output = Float;

    fn div(self, rhs: TimeSpan) -> Self::Output {
        self.0 / rhs.0
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct Rate(Float);

impl Debug for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

impl Rate {
    #[must_use]
    pub const fn new(r: Float) -> Rate {
        Rate(r)
    }

    #[must_use]
    pub fn period(&self) -> TimeSpan {
        TimeSpan::new(1. / self.0)
    }

    #[must_use]
    pub const fn value(&self) -> Float {
        self.0
    }
}

impl Add<Rate> for Rate {
    type Output = Rate;

    fn add(self, rhs: Rate) -> Self::Output {
        Rate(self.0 + rhs.0)
    }
}

impl Div<TimeSpan> for Float {
    type Output = Rate;

    fn div(self, rhs: TimeSpan) -> Self::Output {
        Rate(self / rhs.0)
    }
}

impl Div<Float> for Rate {
    type Output = Rate;

    fn div(self, rhs: Float) -> Self::Output {
        Rate::new(self.0 / rhs)
    }
}

impl Display for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}s^-1", self.0)
    }
}

#[derive(PartialEq, Clone, Copy)]
pub struct Time(Float);

impl Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

impl Time {
    pub const MIN: Time = Time(Float::MIN);
    pub const MAX: Time = Time(Float::MAX);

    #[must_use]
    pub const fn from_sim_start(t: Float) -> Time {
        Time(t)
    }

    #[must_use]
    pub const fn sim_start() -> Time {
        Time::from_sim_start(0.)
    }
}

impl Eq for Time {}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Sub<Time> for Time {
    type Output = TimeSpan;

    fn sub(self, Time(t): Time) -> Self::Output {
        TimeSpan::new(self.0 - t)
    }
}

impl Add<TimeSpan> for Time {
    type Output = Time;

    fn add(self, TimeSpan(ts): TimeSpan) -> Self::Output {
        Time::from_sim_start(self.0 + ts)
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
            (Some(Time(t1)), Some(Time(t2))) => Some(Time::from_sim_start(Float::min(t1, t2))),
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
            (Some(Time(t1)), Some(Time(t2))) => Some(Time::from_sim_start(Float::max(t1, t2))),
            (m, None) | (None, m) => m,
        })
}
