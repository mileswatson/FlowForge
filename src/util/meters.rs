use std::{
    cell::RefCell,
    fmt::Debug,
    ops::{Add, Mul},
};

use crate::{
    flow::{FlowNeverActive, FlowProperties, NoPacketsAcked},
    quantities::{bits_per_second, Float, Information, InformationRate, Time, TimeSpan},
};

use super::average::Average;

#[derive(Clone, Debug)]
pub struct Mean<T>
where
    T: Average,
{
    aggregator: T::Aggregator,
}

impl<T> Mean<T>
where
    T: Average,
    T::Aggregator: Clone,
{
    #[must_use]
    pub fn new() -> Mean<T> {
        Mean::default()
    }

    pub fn record(&mut self, value: T) {
        self.aggregator = T::aggregate(self.aggregator.clone(), value);
    }

    #[must_use]
    pub fn value(&self) -> T::Output {
        T::average(self.aggregator.clone())
    }
}

impl<T> Default for Mean<T>
where
    T: Average,
{
    fn default() -> Self {
        Mean {
            aggregator: T::new_aggregator(),
        }
    }
}

#[derive(Debug)]
pub struct EWMA<T> {
    update_weight: Float,
    current: Option<T>,
}

impl<T> EWMA<T>
where
    T: Add<T, Output = T> + Copy,
    Float: Mul<T, Output = T>,
{
    #[must_use]
    pub const fn new(update_weight: Float) -> EWMA<T> {
        EWMA {
            update_weight,
            current: None,
        }
    }

    pub fn update(&mut self, value: T) -> T {
        let new_value = match self.current {
            Some(current) => (1. - self.update_weight) * current + self.update_weight * value,
            None => value,
        };
        self.current = Some(new_value);
        new_value
    }

    pub const fn value(&self) -> Option<T> {
        self.current
    }
}

#[derive(Debug)]
pub struct TimeBasedEWMA<T> {
    half_life: TimeSpan,
    current: Option<(Time, T)>,
    default: Option<T>,
}

impl<T> TimeBasedEWMA<T>
where
    T: Add<T, Output = T> + Copy,
    Float: Mul<T, Output = T> + Mul<Float, Output = Float>,
{
    #[must_use]
    pub fn new(half_life: TimeSpan, default: Option<(Time, T)>) -> TimeBasedEWMA<T> {
        TimeBasedEWMA {
            half_life,
            default: default.map(|(_, x)| x),
            current: default,
        }
    }

    pub fn update(&mut self, value: T, time: Time) -> T {
        let new_value = match self.current {
            Some((last_time, current)) => {
                let alpha: Float = 1.
                    - <Float as Mul<Float>>::mul(
                        (0.5_f64).ln(),
                        (time - last_time) / self.half_life,
                    )
                    .exp();
                (1. - alpha) * current + alpha * value
            }
            None => value,
        };
        self.current = Some((time, new_value));
        new_value
    }

    pub fn value(&self, time: Time) -> Option<T> {
        self.current.map(|(last_time, current)| {
            assert!(time >= last_time);
            self.default.map_or(current, |default| {
                let alpha: Float = 1.
                    - <Float as Mul<Float>>::mul(
                        (0.5_f64).ln(),
                        (time - last_time) / self.half_life,
                    )
                    .exp();
                alpha * default + (1. - alpha) * current
            })
        })
    }
}

#[derive(Clone, Debug)]
pub struct Timer {
    total_time: TimeSpan,
    current_start: Option<Time>,
}

impl Timer {
    #[must_use]
    pub fn new_enabled(time: Time) -> Timer {
        let mut t = Self::new_disabled();
        t.enable(time);
        t
    }

    #[must_use]
    pub const fn new_disabled() -> Timer {
        Timer {
            total_time: TimeSpan::ZERO,
            current_start: None,
        }
    }

    pub fn enable(&mut self, time: Time) {
        assert!(self.current_start.is_none(), "Timer already enabled!");
        self.current_start = Some(time);
    }

    pub fn disable(&mut self, time: Time) {
        let current_start = self.current_start.expect("Timer already disabled!");
        assert!(time >= current_start);
        self.total_time = self.total_time + (time - current_start);
        self.current_start = None;
    }

    #[must_use]
    pub fn current_value(&self, time: Time) -> TimeSpan {
        self.total_time + self.current_start.map_or(TimeSpan::ZERO, |x| time - x)
    }
}

#[derive(Clone, Debug)]
pub struct InfoRateMeter {
    timer: Timer,
    total: Information,
}

fn calculate_rate(
    total: Information,
    enabled_time: TimeSpan,
) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
    assert!(!enabled_time.is_negative());
    if enabled_time == TimeSpan::ZERO {
        return Err(InfoRateMeterNeverEnabled);
    }
    #[allow(clippy::cast_precision_loss)]
    return Ok(total / enabled_time);
}

pub struct InfoRateMeterNeverEnabled;

impl InfoRateMeter {
    #[must_use]
    pub fn new_enabled(time: Time) -> InfoRateMeter {
        let mut m = Self::new_disabled();
        m.enable(time);
        m
    }

    #[must_use]
    pub const fn new_disabled() -> InfoRateMeter {
        InfoRateMeter {
            timer: Timer::new_disabled(),
            total: Information::ZERO,
        }
    }

    pub fn record_info(&mut self, info: Information) {
        self.total = self.total + info;
    }

    pub fn current_value(&self, time: Time) -> Result<InformationRate, InfoRateMeterNeverEnabled> {
        calculate_rate(self.total, self.timer.current_value(time))
    }

    pub fn enable(&mut self, time: Time) {
        self.timer.enable(time);
    }

    pub fn disable(&mut self, time: Time) {
        self.timer.disable(time);
    }
}

pub trait FlowMeter: Debug {
    fn set_enabled(&mut self, time: Time);
    fn set_disabled(&mut self, time: Time);
    fn packet_received(&mut self, data: Information, rtt: TimeSpan, time: Time);
}

impl<T> FlowMeter for &mut T
where
    T: FlowMeter,
{
    fn set_enabled(&mut self, time: Time) {
        (*self).set_enabled(time);
    }

    fn set_disabled(&mut self, time: Time) {
        (*self).set_disabled(time);
    }

    fn packet_received(&mut self, data: Information, rtt: TimeSpan, time: Time) {
        (*self).packet_received(data, rtt, time);
    }
}

impl<T> FlowMeter for &RefCell<T>
where
    T: FlowMeter,
{
    fn set_enabled(&mut self, time: Time) {
        self.borrow_mut().set_enabled(time);
    }

    fn set_disabled(&mut self, time: Time) {
        self.borrow_mut().set_disabled(time);
    }

    fn packet_received(&mut self, data: Information, rtt: TimeSpan, time: Time) {
        self.borrow_mut().packet_received(data, rtt, time);
    }
}

impl<T, U> FlowMeter for (T, U)
where
    T: FlowMeter,
    U: FlowMeter,
{
    fn set_enabled(&mut self, time: Time) {
        self.0.set_enabled(time);
        self.1.set_enabled(time);
    }

    fn set_disabled(&mut self, time: Time) {
        self.0.set_disabled(time);
        self.1.set_disabled(time);
    }

    fn packet_received(&mut self, data: Information, rtt: TimeSpan, time: Time) {
        self.0.packet_received(data, rtt, time);
        self.1.packet_received(data, rtt, time);
    }
}

#[derive(Debug)]
pub struct NoFlowMeter;

impl FlowMeter for NoFlowMeter {
    fn set_enabled(&mut self, _time: Time) {}

    fn set_disabled(&mut self, _time: Time) {}

    fn packet_received(&mut self, _data: Information, _rtt: TimeSpan, _time: Time) {}
}

#[derive(Debug)]
pub struct AverageFlowMeter {
    average_throughput: InfoRateMeter,
    average_rtt: Mean<TimeSpan>,
}

impl AverageFlowMeter {
    #[must_use]
    pub fn new_enabled(current_time: Time) -> AverageFlowMeter {
        let mut t = Self::new_disabled();
        t.set_enabled(current_time);
        t
    }

    #[must_use]
    pub fn new_disabled() -> AverageFlowMeter {
        AverageFlowMeter {
            average_throughput: InfoRateMeter::new_disabled(),
            average_rtt: Mean::new(),
        }
    }

    pub fn average_properties(
        &self,
        current_time: Time,
    ) -> Result<FlowProperties, FlowNeverActive> {
        self.average_throughput
            .current_value(current_time)
            .map(|average_throughput| FlowProperties {
                throughput: average_throughput,
                rtt: self.average_rtt.value().map_err(|_| NoPacketsAcked),
            })
            .map_err(|_| FlowNeverActive)
    }
}

impl FlowMeter for AverageFlowMeter {
    fn set_enabled(&mut self, time: Time) {
        self.average_throughput.enable(time);
    }

    fn set_disabled(&mut self, time: Time) {
        self.average_throughput.disable(time);
    }

    fn packet_received(&mut self, data: Information, rtt: TimeSpan, _time: Time) {
        self.average_throughput.record_info(data);
        self.average_rtt.record(rtt);
    }
}

#[derive(Debug)]
pub struct CurrentFlowMeter {
    current_throughput: TimeBasedEWMA<InformationRate>,
    current_rtt: TimeBasedEWMA<TimeSpan>,
    last_received: Time,
    enabled: bool,
}

#[derive(Clone, Debug)]
pub struct FlowNotActive;

impl CurrentFlowMeter {
    #[must_use]
    pub fn new_enabled(current_time: Time, half_life: TimeSpan) -> CurrentFlowMeter {
        let mut t = Self::new_disabled(current_time, half_life);
        t.set_enabled(current_time);
        t
    }

    #[must_use]
    pub fn new_disabled(current_time: Time, half_life: TimeSpan) -> CurrentFlowMeter {
        CurrentFlowMeter {
            current_throughput: TimeBasedEWMA::new(
                half_life,
                Some((current_time, bits_per_second(0.))),
            ),
            current_rtt: TimeBasedEWMA::new(half_life, None),
            last_received: current_time,
            enabled: false,
        }
    }

    #[must_use]
    pub fn current_bandwidth(&self, time: Time) -> InformationRate {
        self.current_throughput.value(time).unwrap()
    }

    pub fn current_rtt(&self, time: Time) -> Result<TimeSpan, NoPacketsAcked> {
        self.current_rtt.value(time).ok_or(NoPacketsAcked)
    }

    pub fn current_properties(&self, current_time: Time) -> Result<FlowProperties, FlowNotActive> {
        if self.enabled {
            Ok(FlowProperties {
                throughput: self.current_throughput.value(current_time).unwrap(),
                rtt: self.current_rtt.value(current_time).ok_or(NoPacketsAcked),
            })
        } else {
            Err(FlowNotActive)
        }
    }

    #[must_use]
    pub const fn active(&self) -> bool {
        self.enabled
    }
}

impl FlowMeter for CurrentFlowMeter {
    fn set_enabled(&mut self, _time: Time) {
        self.enabled = true;
    }

    fn set_disabled(&mut self, _time: Time) {
        self.enabled = false;
    }

    fn packet_received(&mut self, data: Information, rtt: TimeSpan, time: Time) {
        assert!(time > self.last_received);
        self.current_throughput
            .update(data / (time - self.last_received), time);
        self.current_rtt.update(rtt, time);
        self.last_received = time;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        quantities::{seconds, Time},
        util::meters::TimeBasedEWMA,
    };

    use super::{Mean, EWMA};

    #[test]
    pub fn mean() {
        let mut mean = Mean::<f64>::new();
        assert!(mean.value().is_err());
        mean.record(3.);
        assert_eq!(mean.value(), Ok(3.));
        mean.record(5.);
        assert_eq!(mean.value(), Ok(4.));
    }

    #[test]
    pub fn ewma() {
        let mut ewma = EWMA::<f64>::new(0.1);
        assert!(ewma.value().is_none());
        ewma.update(10.);
        assert_eq!(ewma.value(), Some(10.));
        ewma.update(20.);
        assert_eq!(ewma.value(), Some(11.));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    pub fn time_based_ewma() {
        let mut ewma = TimeBasedEWMA::<f64>::new(seconds(1.), None);
        assert!(ewma.value(Time::from_sim_start(seconds(2.))).is_none());
        assert_eq!(ewma.update(10., Time::from_sim_start(seconds(0.))), 10.);
        assert_eq!(ewma.value(Time::from_sim_start(seconds(2.))), Some(10.));
        assert_eq!(ewma.update(20., Time::from_sim_start(seconds(1.))), 15.);
        assert_eq!(ewma.value(Time::from_sim_start(seconds(2.))), Some(15.));

        let mut ewma =
            TimeBasedEWMA::<f64>::new(seconds(1.), Some((Time::from_sim_start(seconds(0.)), 0.)));
        assert_eq!(ewma.value(Time::from_sim_start(seconds(2.))), Some(0.));
        assert_eq!(ewma.update(6., Time::from_sim_start(seconds(1.))), 3.);
        assert_eq!(ewma.value(Time::from_sim_start(seconds(2.))), Some(1.5));
        assert_eq!(ewma.update(10., Time::from_sim_start(seconds(2.))), 6.5);
        assert_eq!(ewma.value(Time::from_sim_start(seconds(2.))), Some(6.5));
        assert_eq!(ewma.value(Time::from_sim_start(seconds(3.))), Some(3.25));
    }
}
