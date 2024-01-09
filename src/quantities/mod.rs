use std::{
    any::type_name,
    fmt::{self, Display},
    marker::PhantomData,
    ops::{Div, Mul, Rem},
    str::FromStr,
};

pub type Float = f64;

pub mod information;
pub mod information_rate;
pub mod time;
pub mod time_span;

pub use information::*;
pub use information_rate::*;
use itertools::Itertools;
use serde::de::{self, Visitor};
pub use time::*;
pub use time_span::*;

use crate::rand::Wrapper;

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

pub struct NoMatch;

pub trait UnitPrefix<U> {
    fn symbol(&self) -> &'static str;
    fn parsed_to_underlying(&self, value: U) -> U;
    fn quantity_to_parseable(&self, value: U) -> Result<U, NoMatch>;
}

pub trait Quantity: Wrapper + Copy + 'static {
    const BASE_UNIT: &'static str;
    const UNIT_PREFIXES: &'static [&'static dyn UnitPrefix<Self::Underlying>];
}

struct QuantityVisitor<Q>(PhantomData<Q>);

impl<'de, Q> Visitor<'de> for QuantityVisitor<Q>
where
    Q: Quantity,
    Q::Underlying: FromStr,
{
    type Value = Q;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between -2^31 and 2^31")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let error = || {
            let formats = Q::UNIT_PREFIXES
                .iter()
                .map(|p| p.symbol().to_owned() + Q::BASE_UNIT)
                .join(", ");
            E::custom(format!("Quantity does not end with one of {formats}"))
        };
        let v = v.strip_suffix(Q::BASE_UNIT).ok_or_else(error)?;
        for unit_prefix in Q::UNIT_PREFIXES {
            if let Some(v) = v.strip_suffix(unit_prefix.symbol()) {
                return v
                    .parse::<Q::Underlying>()
                    .map(|u| Q::from_underlying(unit_prefix.parsed_to_underlying(u)))
                    .map_err(|_| {
                        E::custom(format!(
                            "Failed to parse string to {}",
                            type_name::<Q::Underlying>()
                        ))
                    });
            }
        }
        Err(error())
    }
}

fn serialize<Q, S>(quantity: &Q, serializer: S) -> Result<S::Ok, S::Error>
where
    Q: Quantity,
    Q::Underlying: Display,
    S: serde::Serializer,
{
    for unit_prefix in Q::UNIT_PREFIXES {
        if let Ok(u) = unit_prefix.quantity_to_parseable(quantity.to_underlying()) {
            return serializer.serialize_str(&format!(
                "{}{}{}",
                u,
                unit_prefix.symbol(),
                Q::BASE_UNIT
            ));
        }
    }
    panic!("Failed to serialize!");
}

fn deserialize<'de, Q, D>(deserializer: D) -> Result<Q, D::Error>
where
    Q: Quantity,
    Q::Underlying: FromStr,
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(QuantityVisitor::<Q>(PhantomData))
}

struct Uno;

impl UnitPrefix<Float> for Uno {
    fn symbol(&self) -> &'static str {
        ""
    }
    fn parsed_to_underlying(&self, value: Float) -> Float {
        value
    }

    fn quantity_to_parseable(&self, value: Float) -> Result<Float, NoMatch> {
        Ok(value)
    }
}

struct Milli;

impl UnitPrefix<Float> for Milli {
    fn symbol(&self) -> &'static str {
        "m"
    }
    fn parsed_to_underlying(&self, value: Float) -> Float {
        value / 1000.
    }

    fn quantity_to_parseable(&self, value: Float) -> Result<Float, NoMatch> {
        if value < 1. {
            Ok(value * 1000.)
        } else {
            Err(NoMatch)
        }
    }
}

pub struct Kilo;

impl<U> UnitPrefix<U> for Kilo
where
    U: From<u32> + Mul<U, Output = U> + Div<U, Output = U> + Rem<U, Output = U> + PartialEq + Copy,
{
    fn symbol(&self) -> &'static str {
        "k"
    }

    fn parsed_to_underlying(&self, value: U) -> U {
        value * 1000.into()
    }

    fn quantity_to_parseable(&self, value: U) -> Result<U, NoMatch> {
        if value % 1000.into() == 0.into() {
            Ok(value / 1000.into())
        } else {
            Err(NoMatch)
        }
    }
}

pub struct Mega;

impl<U> UnitPrefix<U> for Mega
where
    U: From<u32> + Mul<U, Output = U> + Div<U, Output = U> + Rem<U, Output = U> + PartialEq + Copy,
{
    fn symbol(&self) -> &'static str {
        "M"
    }

    fn parsed_to_underlying(&self, value: U) -> U {
        value * 1_000_000.into()
    }

    fn quantity_to_parseable(&self, value: U) -> Result<U, NoMatch> {
        if value % 1_000_000.into() == 0.into() {
            Ok(value / 1_000_000.into())
        } else {
            Err(NoMatch)
        }
    }
}

pub struct Giga;

impl<U> UnitPrefix<U> for Giga
where
    U: From<u32> + Mul<U, Output = U> + Div<U, Output = U> + Rem<U, Output = U> + PartialEq + Copy,
{
    fn symbol(&self) -> &'static str {
        "G"
    }

    fn parsed_to_underlying(&self, value: U) -> U {
        value * 1_000_000_000.into()
    }

    fn quantity_to_parseable(&self, value: U) -> Result<U, NoMatch> {
        if value % 1_000_000_000.into() == 0.into() {
            Ok(value / 1_000_000_000.into())
        } else {
            Err(NoMatch)
        }
    }
}
