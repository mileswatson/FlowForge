use std::{
    fmt::Display,
    ops::{Add, Div, Mul, MulAssign, Sub},
};

use format_num::format_num;
use serde::{Deserialize, Serialize};

use crate::util::rand::Wrapper;

use super::{deserialize, serialize, Float, Milli, Quantity, UnitPrefix, Uno};

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct TimeSpan(Float);

impl Eq for TimeSpan {}

impl PartialOrd for TimeSpan {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeSpan {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl TimeSpan {
    pub const ZERO: TimeSpan = TimeSpan(0.);
    pub const MIN: TimeSpan = TimeSpan(Float::MIN);
    pub const MAX: TimeSpan = TimeSpan(Float::MAX);

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

impl Wrapper for TimeSpan {
    type Underlying = Float;

    fn from_underlying(value: Self::Underlying) -> Self {
        TimeSpan(value)
    }

    fn to_underlying(self) -> Self::Underlying {
        self.0
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

impl Sub for TimeSpan {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        TimeSpan(self.0 - rhs.0)
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

impl Quantity for TimeSpan {
    const BASE_UNIT: &'static str = "s";
    const UNIT_PREFIXES: &'static [&'static dyn UnitPrefix<Float>] = &[&Milli, &Uno];
}

impl Serialize for TimeSpan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for TimeSpan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}
