use protobuf::MessageField;

use crate::time::Float;

use super::autogen::remy_dna::Memory;

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub ack_ewma: Float,
    pub send_ewma: Float,
    pub rtt_ratio: Float,
}

impl Point {
    pub const MIN: Point = Point {
        ack_ewma: 0.,
        send_ewma: 0.,
        rtt_ratio: 0.,
    };
    // TODO
    pub const MAX: Point = Point {
        ack_ewma: 163_840.,
        send_ewma: 163_840.,
        rtt_ratio: 163_840.,
    };
}

impl From<Point> for Memory {
    fn from(value: Point) -> Self {
        let mut memory = Memory::new();
        memory.set_rec_rec_ewma(value.ack_ewma);
        memory.set_rec_send_ewma(value.send_ewma);
        memory.set_rtt_ratio(value.rtt_ratio);
        memory
    }
}

impl From<MessageField<Memory>> for Point {
    fn from(value: MessageField<Memory>) -> Self {
        Point {
            ack_ewma: value.rec_rec_ewma(),
            send_ewma: value.rec_send_ewma(),
            rtt_ratio: value.rtt_ratio(),
        }
    }
}
