use std::{
    fmt::Display,
    ops::{Add, Div},
};

use serde::{Deserialize, Serialize};

use crate::rand::Wrapper;

use super::{
    deserialize, packets, serialize, Float, Giga, Kilo, Mega, Milli, Quantity, UnitPrefix, Uno,
};

#[derive(PartialEq, PartialOrd, Clone, Copy, Debug)]
pub struct InformationRate(Float);

impl Wrapper for InformationRate {
    type Underlying = Float;

    fn from_underlying(value: Self::Underlying) -> Self {
        InformationRate(value)
    }

    fn to_underlying(self) -> Self::Underlying {
        self.0
    }
}

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

impl Quantity for InformationRate {
    const BASE_UNIT: &'static str = "b/s";
    const UNIT_PREFIXES: &'static [&'static dyn UnitPrefix<Float>] =
        &[&Giga, &Mega, &Kilo, &Milli, &Uno];
}

impl Serialize for InformationRate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for InformationRate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}
