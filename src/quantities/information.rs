use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use serde::{Deserialize, Serialize};

use crate::util::rand::Wrapper;

use super::{deserialize, display, serialize, Float, Giga, Kilo, Mega, Quantity, UnitPrefix, Uno};

#[derive(PartialEq, Eq, PartialOrd, Clone, Copy, Debug)]
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

impl Sub<Information> for Information {
    type Output = Information;

    fn sub(self, rhs: Information) -> Self::Output {
        Information(self.0 - rhs.0)
    }
}

impl Wrapper for Information {
    type Underlying = u64;

    fn from_underlying(value: Self::Underlying) -> Self {
        Information(value)
    }

    fn to_underlying(self) -> Self::Underlying {
        self.0
    }
}

impl Quantity for Information {
    const BASE_UNIT: &'static str = "B";
    const UNIT_PREFIXES: &'static [&'static dyn UnitPrefix<u64>] = &[&Giga, &Mega, &Kilo, &Uno];
}

impl Serialize for Information {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Information {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

impl Display for Information {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(clippy::cast_precision_loss)]
        display(self, f, |i| i as Float)
    }
}
