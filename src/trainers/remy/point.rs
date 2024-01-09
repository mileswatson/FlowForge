use std::fmt::Display;

use protobuf::MessageField;

use crate::quantities::{milliseconds, seconds, Float, TimeSpan};

use super::autogen::remy_dna::Memory;

#[derive(Debug, Clone, PartialEq)]
pub struct Point<const TESTING: bool = false> {
    pub ack_ewma: TimeSpan,
    pub send_ewma: TimeSpan,
    pub rtt_ratio: Float,
}

impl<const TESTING: bool> Display for Point<TESTING> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Point {{ ack_ewma: {}, send_ewma: {}, rtt_ratio: {} }}",
            self.ack_ewma, self.send_ewma, self.rtt_ratio
        )
    }
}

impl<const TESTING: bool> Point<TESTING> {
    pub const MIN: Self = Point {
        ack_ewma: seconds(0.),
        send_ewma: seconds(0.),
        rtt_ratio: 0.,
    };
    // TODO
    pub const MAX: Self = Point {
        ack_ewma: seconds(600.),
        send_ewma: seconds(600.),
        rtt_ratio: 1000.,
    };

    #[must_use]
    pub fn from_memory(memory: &MessageField<Memory>) -> Self {
        let convert = |x| if TESTING { seconds(x) } else { milliseconds(x) };
        Point {
            ack_ewma: convert(memory.rec_rec_ewma()),
            send_ewma: convert(memory.rec_send_ewma()),
            rtt_ratio: memory.rtt_ratio(),
        }
    }

    #[must_use]
    pub fn to_memory(&self) -> Memory {
        let convert = |x: TimeSpan| {
            if TESTING {
                x.seconds()
            } else {
                x.milliseconds()
            }
        };
        let mut memory = Memory::new();
        memory.set_rec_rec_ewma(convert(self.ack_ewma));
        memory.set_rec_send_ewma(convert(self.send_ewma));
        memory.set_rtt_ratio(self.rtt_ratio);
        memory
    }
}
