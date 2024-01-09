use crate::time::Quantity;

use super::point::Point;

use std::fmt::Display;

#[derive(Clone, Debug, PartialEq)]
pub struct Cube<const TESTING: bool = false> {
    pub min: Point<TESTING>,
    pub max: Point<TESTING>,
}

impl<const TESTING: bool> Display for Cube<TESTING> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cube {{ ack_ewma: {}-{}, send_ewma: {}-{}, rtt_ratio: {:.4}-{:.4} }}",
            self.min.ack_ewma.display(),
            self.max.ack_ewma.display(),
            self.min.send_ewma.display(),
            self.max.send_ewma.display(),
            self.min.rtt_ratio,
            self.max.rtt_ratio
        )
    }
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            min: Point::min(),
            max: Point::max(),
        }
    }
}

fn within<T>(min: &T, x: &T, max: &T) -> bool
where
    T: PartialOrd,
{
    min <= x && x < max
}

impl<const TESTING: bool> Cube<TESTING> {
    #[must_use]
    pub fn contains(&self, point: &Point) -> bool {
        within(&self.min.rtt_ratio, &point.rtt_ratio, &self.max.rtt_ratio)
            && within(&self.min.ack_ewma, &point.ack_ewma, &self.max.ack_ewma)
            && within(&self.min.send_ewma, &point.send_ewma, &self.max.send_ewma)
    }

    fn split_ack_ewma(&self) -> Vec<Cube<TESTING>> {
        let ack_ewma = (self.max.ack_ewma + self.min.ack_ewma) / 2.;
        vec![
            Cube {
                min: self.min.clone(),
                max: Point {
                    ack_ewma,
                    ..self.max
                },
            },
            Cube {
                min: Point {
                    ack_ewma,
                    ..self.min
                },
                max: self.max.clone(),
            },
        ]
    }

    fn split_send_ewma(&self) -> Vec<Cube<TESTING>> {
        let send_ewma = (self.max.send_ewma + self.min.send_ewma) / 2.;
        vec![
            Cube {
                min: self.min.clone(),
                max: Point {
                    send_ewma,
                    ..self.max
                },
            },
            Cube {
                min: Point {
                    send_ewma,
                    ..self.min
                },
                max: self.max.clone(),
            },
        ]
    }

    fn split_rtt_ratio(&self) -> Vec<Cube<TESTING>> {
        let rtt_ratio = (self.max.rtt_ratio + self.min.rtt_ratio) / 2.;
        vec![
            Cube {
                min: self.min.clone(),
                max: Point {
                    rtt_ratio,
                    ..self.max
                },
            },
            Cube {
                min: Point {
                    rtt_ratio,
                    ..self.min
                },
                max: self.max.clone(),
            },
        ]
    }

    #[must_use]
    pub fn split(&self) -> Vec<Cube<TESTING>> {
        self.split_ack_ewma()
            .into_iter()
            .flat_map(|x| x.split_send_ewma())
            .flat_map(|x| x.split_rtt_ratio())
            .collect()
    }
}
