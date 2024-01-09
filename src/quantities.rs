use std::{
    fmt::Debug,
    fmt::Display,
    ops::{Add, Div, Mul, MulAssign, Sub},
};

use format_num::format_num;
use serde::{Deserialize, Serialize};

pub type Float = f64;

#[derive(PartialEq, PartialOrd, Clone, Copy, Serialize, Deserialize, Debug)]
pub struct TimeSpan(Float);

impl TimeSpan {
    pub const ZERO: TimeSpan = TimeSpan(0.);

    #[must_use]
    pub const fn seconds(self) -> Float {
        self.0
    }

    #[must_use]
    pub fn milliseconds(self) -> Float {
        self.0 * 1000.
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

#[must_use]
pub const fn seconds(value: Float) -> TimeSpan {
    TimeSpan(value)
}

#[must_use]
pub fn milliseconds(value: Float) -> TimeSpan {
    seconds(value / 1000.)
}

impl Add for TimeSpan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TimeSpan(self.0 + rhs.0)
    }
}

impl Mul<TimeSpan> for Float {
    type Output = TimeSpan;

    fn mul(self, rhs: TimeSpan) -> Self::Output {
        TimeSpan(self * rhs.0)
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
        TimeSpan(self.0 / rhs)
    }
}

impl Div<TimeSpan> for TimeSpan {
    type Output = Float;

    fn div(self, rhs: TimeSpan) -> Self::Output {
        self.0 / rhs.0
    }
}

#[derive(PartialEq, Eq, PartialOrd, Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Information(u64);

impl Information {
    pub const ZERO: Information = Information(0);

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bytes() * 8
    }

    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.0
    }
}

#[must_use]
pub const fn bytes(value: u64) -> Information {
    Information(value)
}

#[must_use]
pub const fn packets(value: u64) -> Information {
    bytes(1400 * value)
}

impl Add<Information> for Information {
    type Output = Information;

    fn add(self, rhs: Information) -> Self::Output {
        Information(self.0 + rhs.0)
    }
}

impl Div<InformationRate> for Information {
    type Output = TimeSpan;

    fn div(self, rhs: InformationRate) -> Self::Output {
        #[allow(clippy::cast_precision_loss)]
        seconds(self.bits() as Float / rhs.bits_per_second())
    }
}

impl Div<TimeSpan> for Information {
    type Output = InformationRate;

    fn div(self, rhs: TimeSpan) -> Self::Output {
        #[allow(clippy::cast_precision_loss)]
        bits_per_second(self.bits() as Float / rhs.seconds())
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy, Serialize, Deserialize, Debug)]
pub struct InformationRate(Float);

impl InformationRate {
    #[must_use]
    pub const fn value(&self) -> Float {
        self.0
    }

    #[must_use]
    pub const fn bits_per_second(self) -> Float {
        self.0
    }
}

#[must_use]
pub const fn bits_per_second(r: Float) -> InformationRate {
    InformationRate(r)
}

#[must_use]
pub fn packets_per_second(value: Float) -> InformationRate {
    #[allow(clippy::cast_precision_loss)]
    bits_per_second(value * packets(1).bits() as Float)
}

impl Add<InformationRate> for InformationRate {
    type Output = InformationRate;

    fn add(self, rhs: InformationRate) -> Self::Output {
        InformationRate(self.0 + rhs.0)
    }
}

impl Div<Float> for InformationRate {
    type Output = InformationRate;

    fn div(self, rhs: Float) -> Self::Output {
        InformationRate(self.0 / rhs)
    }
}

impl Display for InformationRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}bps", self.0)
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
        TimeSpan(self.0 - t)
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
