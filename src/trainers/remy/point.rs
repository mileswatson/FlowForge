use protobuf::MessageField;
use uom::{
    si::{
        f64::Time,
        time::{millisecond, second},
    },
    ConstZero,
};

use crate::time::Float;

use super::autogen::remy_dna::Memory;

#[derive(Debug, Clone, PartialEq)]
pub struct Point<const TESTING: bool = false> {
    pub ack_ewma: Time,
    pub send_ewma: Time,
    pub rtt_ratio: Float,
}

impl<const TESTING: bool> Point<TESTING> {
    #[must_use]
    pub const fn min() -> Self {
        Point {
            ack_ewma: Time::ZERO,
            send_ewma: Time::ZERO,
            rtt_ratio: 0.,
        }
    }
    // TODO
    #[must_use]
    pub fn max() -> Self {
        Point {
            ack_ewma: Time::new::<second>(163_840.),
            send_ewma: Time::new::<second>(163_840.),
            rtt_ratio: 163_840.,
        }
    }

    #[must_use]
    pub fn from_memory(memory: &MessageField<Memory>) -> Self {
        let convert = |x| {
            if TESTING {
                Time::new::<second>(x)
            } else {
                Time::new::<millisecond>(x)
            }
        };
        Point {
            ack_ewma: convert(memory.rec_rec_ewma()),
            send_ewma: convert(memory.rec_send_ewma()),
            rtt_ratio: memory.rtt_ratio(),
        }
    }

    #[must_use]
    pub fn to_memory(&self) -> Memory {
        let convert = |x: Time| {
            if TESTING {
                x.get::<second>()
            } else {
                x.get::<millisecond>()
            }
        };
        let mut memory = Memory::new();
        memory.set_rec_rec_ewma(convert(self.ack_ewma));
        memory.set_rec_send_ewma(convert(self.send_ewma));
        memory.set_rtt_ratio(self.rtt_ratio);
        memory
    }
}
