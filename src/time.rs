use std::{
    fmt::Debug,
    ops::{Add, Sub},
};

use format_num::format_num;
use uom::si::{
    f64::{self, InformationRate, Time},
    information::bit,
    information_rate::bit_per_second,
    time::second,
    u64::Information,
};

pub mod packet {
    unit! {
        system: uom::si;
        quantity: uom::si::information;

        @packet: 1450.; "p", "packet", "packets";
    }
}

pub mod packet_per_second {
    unit! {
        system: uom::si;
        quantity: uom::si::information_rate;

        @packet_per_second: 1450.; "p/s", "packet", "packets";
    }
}

pub type Float = f64;

pub trait Quantity {
    fn display(&self) -> String;
}

impl Quantity for Time {
    fn display(&self) -> String {
        format!("{}s", format_num!(".3s", self.get::<second>()))
    }
}

impl Quantity for TimePoint {
    fn display(&self) -> String {
        format!("{}t", format_num!(".4f", self.0.get::<second>()))
    }
}

impl Quantity for Information {
    fn display(&self) -> String {
        #[allow(clippy::cast_precision_loss)]
        return format!("{}b", format_num!(".3s", self.get::<bit>() as Float));
    }
}

impl Quantity for InformationRate {
    fn display(&self) -> String {
        format!("{}bps", format_num!(".3s", self.get::<bit_per_second>()))
    }
}

#[must_use]
pub fn transmission_time(information: Information, rate: InformationRate) -> Time {
    #[allow(clippy::cast_precision_loss)]
    return f64::Information::new::<bit>(information.get::<bit>() as Float) / rate;
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct TimePoint(Time);

impl PartialOrd for TimePoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimePoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

impl Eq for TimePoint {}

impl TimePoint {
    #[must_use]
    pub fn min() -> TimePoint {
        TimePoint(Time::new::<second>(Float::MIN))
    }
    #[must_use]
    pub fn max() -> TimePoint {
        TimePoint(Time::new::<second>(Float::MAX))
    }

    #[must_use]
    pub fn from_sim_start(t: Float) -> TimePoint {
        TimePoint(Time::new::<second>(t))
    }

    #[must_use]
    pub fn sim_start() -> TimePoint {
        TimePoint::from_sim_start(0.)
    }
}

impl Sub<TimePoint> for TimePoint {
    type Output = Time;

    fn sub(self, other: TimePoint) -> Self::Output {
        self.0 - other.0
    }
}

impl Add<Time> for TimePoint {
    type Output = TimePoint;

    fn add(self, time: Time) -> Self::Output {
        TimePoint(self.0 + time)
    }
}

#[must_use]
pub fn earliest(times: &[TimePoint]) -> TimePoint {
    times.iter().copied().min().unwrap_or_else(TimePoint::min)
}

#[must_use]
pub fn earliest_opt(times: &[Option<TimePoint>]) -> Option<TimePoint> {
    times
        .iter()
        .fold(None, |prev, current| match (prev, *current) {
            (Some(TimePoint(t1)), Some(TimePoint(t2))) => Some(TimePoint(Time::min(t1, t2))),
            (m, None) | (None, m) => m,
        })
}

#[must_use]
pub fn latest(times: &[TimePoint]) -> TimePoint {
    times.iter().copied().max().unwrap_or_else(TimePoint::max)
}

#[must_use]
pub fn latest_opt(times: &[Option<TimePoint>]) -> Option<TimePoint> {
    times
        .iter()
        .fold(None, |prev, current| match (prev, *current) {
            (Some(TimePoint(t1)), Some(TimePoint(t2))) => Some(TimePoint(Time::max(t1, t2))),
            (m, None) | (None, m) => m,
        })
}
