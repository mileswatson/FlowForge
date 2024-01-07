use protobuf::MessageField;

use crate::time::{Float, TimeSpan};

use super::autogen::remy_dna::Memory;

#[derive(Debug, Clone, PartialEq)]
pub struct Point<const TESTING: bool = false> {
    pub ack_ewma: TimeSpan,
    pub send_ewma: TimeSpan,
    pub rtt_ratio: Float,
}

impl<const TESTING: bool> Point<TESTING> {
    pub const MIN: Self = Point {
        ack_ewma: TimeSpan::new(0.),
        send_ewma: TimeSpan::new(0.),
        rtt_ratio: 0.,
    };
    // TODO
    pub const MAX: Self = Point {
        ack_ewma: TimeSpan::new(163_840.),
        send_ewma: TimeSpan::new(163_840.),
        rtt_ratio: 163_840.,
    };

    #[must_use]
    pub fn from_memory(memory: &MessageField<Memory>) -> Self {
        let convert = |x| if TESTING { x } else { x / 1000. };
        Point {
            ack_ewma: convert(memory.rec_rec_ewma().into()),
            send_ewma: convert(memory.rec_send_ewma().into()),
            rtt_ratio: memory.rtt_ratio(),
        }
    }

    #[must_use]
    pub fn to_memory(&self) -> Memory {
        let convert = |x| if TESTING { x } else { x * 1000. };
        let mut memory = Memory::new();
        memory.set_rec_rec_ewma(convert(self.ack_ewma.value()));
        memory.set_rec_send_ewma(convert(self.send_ewma.value()));
        memory.set_rtt_ratio(self.rtt_ratio);
        memory
    }
}