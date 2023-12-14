use std::ops::{Add, Mul};

use crate::time::{Float, TimeSpan};

pub mod lossy_window;

#[derive(Debug)]
pub struct EWMA<T> {
    update_weight: Float,
    current: T,
}

impl<T> EWMA<T>
where
    T: Add<T, Output = T> + Copy,
    Float: Mul<T, Output = T>,
{
    pub const fn new(update_weight: Float, current: T) -> EWMA<T> {
        EWMA {
            update_weight,
            current,
        }
    }

    pub fn update(&mut self, value: T) {
        self.current = (1. - self.update_weight) * self.current + self.update_weight * value;
    }

    pub const fn value(&self) -> T {
        self.current
    }
}

/*
#[derive(Debug)]
struct Timeout {
    ewma: EWMA<TimeSpan>,
    timeout: TimeSpan,
}

impl Timeout {
    pub fn new(update_weight: Float) -> Timeout {
        Timeout {
            ewma: EWMA::new(update_weight, TimeSpan::new(0.)),
            timeout: TimeSpan::new(1.),
        }
    }

    pub fn received_ack(&mut self, rtt: TimeSpan) {
        self.ewma.update(rtt);
        self.timeout = self.ewma.value();
    }

    pub fn timed_out(&mut self) {
        self.timeout *= 2.;
    }

    pub fn value(&self) -> TimeSpan {
        self.timeout
    }
}*/
