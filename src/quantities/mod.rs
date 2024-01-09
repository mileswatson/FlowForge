use std::ops::Div;

pub type Float = f64;

pub mod information;
pub mod information_rate;
pub mod time;
pub mod time_span;

pub use information::*;
pub use information_rate::*;
pub use time::*;
pub use time_span::*;

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
