use std::fmt::Debug;

use crate::{
    quantities::{latest, Float, Time, TimeSpan},
    util::{logging::Logger, meters::EWMA, rand::Rng},
    AckReceived, Cca, PacketSent,
};

#[derive(Debug)]
pub struct DelayMultiplierCca {
    pub multiplier: Float,
    pub rtt: EWMA<TimeSpan>,
    pub last_send: Option<Time>,
}

impl DelayMultiplierCca {
    #[must_use]
    pub const fn new(multiplier: Float, rtt_update_weight: f64) -> DelayMultiplierCca {
        DelayMultiplierCca {
            multiplier,
            rtt: EWMA::new(rtt_update_weight),
            last_send: None,
        }
    }
}

impl Cca for DelayMultiplierCca {
    fn initial_cwnd(&self, _time: Time) -> u32 {
        1
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        self.last_send.and_then(|last_send| {
            self.rtt
                .value()
                .map(|rtt| latest(&[time, last_send + self.multiplier * rtt]))
        })
    }

    fn tick<L: Logger>(&mut self, _rng: &mut Rng, _logger: &mut L) -> u32 {
        self.last_send = None;
        1
    }

    fn ack_received<L>(
        &mut self,
        AckReceived {
            sent_time,
            received_time,
        }: AckReceived,
        _rng: &mut Rng,
        logger: &mut L,
    ) -> u32
    where
        L: Logger,
    {
        let rtt = self.rtt.update(received_time - sent_time);
        let intersend_delay = self.multiplier * rtt;
        log!(logger, "Updated intersend_delay to {}", intersend_delay);
        0
    }

    fn packet_sent<L: Logger>(
        &mut self,
        packet: PacketSent,
        _rng: &mut Rng,
        _logger: &mut L,
    ) -> u32 {
        self.last_send = Some(packet.sent_time);
        0
    }
}
