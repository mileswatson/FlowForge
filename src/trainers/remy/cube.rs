use crate::time::Float;

use super::point::Point;

#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    pub min: Point,
    pub max: Point,
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            min: Point::MIN,
            max: Point::MAX,
        }
    }
}

fn within(min: Float, x: Float, max: Float) -> bool {
    min <= x && x < max
}

impl Cube {
    #[must_use]
    pub fn contains(&self, point: &Point) -> bool {
        within(self.min.rtt_ratio, point.rtt_ratio, self.max.rtt_ratio)
            && within(self.min.ack_ewma, point.ack_ewma, self.max.ack_ewma)
            && within(self.min.send_ewma, point.send_ewma, self.max.send_ewma)
    }
}
