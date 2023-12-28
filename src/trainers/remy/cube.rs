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

    fn split_ack_ewma(&self) -> Vec<Cube> {
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

    fn split_send_ewma(&self) -> Vec<Cube> {
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

    fn split_rtt_ratio(&self) -> Vec<Cube> {
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
    pub fn split(&self) -> Vec<Cube> {
        self.split_ack_ewma()
            .into_iter()
            .flat_map(|x| x.split_send_ewma())
            .flat_map(|x| x.split_rtt_ratio())
            .collect()
    }
}
