use std::{
    fmt::Display,
    ops::{Add, Mul, MulAssign, Sub},
};

pub type Float = f64;

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct TimeSpan {
    ts: Float,
}

impl TimeSpan {
    #[must_use]
    pub const fn new(ts: Float) -> TimeSpan {
        TimeSpan { ts }
    }
}

impl Add for TimeSpan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TimeSpan::new(self.ts + rhs.ts)
    }
}

impl Mul<TimeSpan> for Float {
    type Output = TimeSpan;

    fn mul(self, rhs: TimeSpan) -> Self::Output {
        TimeSpan::new(self * rhs.ts)
    }
}

impl MulAssign<Float> for TimeSpan {
    fn mul_assign(&mut self, rhs: Float) {
        self.ts *= rhs;
    }
}

impl Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.ts)
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct Rate {
    r: Float,
}

impl Rate {
    #[must_use]
    pub const fn new(r: Float) -> Rate {
        Rate { r }
    }

    #[must_use]
    pub fn period(&self) -> TimeSpan {
        TimeSpan::new(1. / self.r)
    }
}

impl Display for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s^-1", self.r)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Time {
    t: Float,
}

impl Time {
    pub const MIN: Time = Time { t: Float::MIN };

    #[must_use]
    pub const fn from_sim_start(t: Float) -> Time {
        Time { t }
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
        self.t.total_cmp(&other.t)
    }
}

impl Sub<Time> for Time {
    type Output = TimeSpan;

    fn sub(self, Time { t }: Time) -> Self::Output {
        TimeSpan::new(self.t - t)
    }
}

impl Add<TimeSpan> for Time {
    type Output = Time;

    fn add(self, TimeSpan { ts }: TimeSpan) -> Self::Output {
        Time::from_sim_start(self.t + ts)
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}t", self.t)
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
            (Some(Time { t: t1 }), Some(Time { t: t2 })) => {
                Some(Time::from_sim_start(Float::min(t1, t2)))
            }
            (m, None) | (None, m) => m,
        })
}