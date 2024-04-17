use std::fmt::Debug;

use crate::{
    core::{logging::Logger, meters::EWMA, rand::Rng},
    quantities::{Float, TimeSpan},
};

use super::window::{AckReceived, Cca, LossyWindowSettings};

#[derive(Debug)]
pub struct DelayMultiplierCca {
    pub multiplier: Float,
    pub rtt: EWMA<TimeSpan>,
}

impl Cca for DelayMultiplierCca {
    fn initial_settings(&self) -> LossyWindowSettings {
        LossyWindowSettings {
            window: 1,
            intersend_delay: TimeSpan::ZERO,
        }
    }

    fn ack_received<L>(
        &mut self,
        AckReceived {
            current_settings,
            sent_time,
            received_time,
        }: AckReceived,
        _rng: &mut Rng,
        logger: &mut L,
    ) -> Option<LossyWindowSettings>
    where
        L: Logger,
    {
        let rtt = self.rtt.update(received_time - sent_time);
        let intersend_delay = self.multiplier * rtt;
        log!(logger, "Updated intersend_delay to {}", intersend_delay);
        Some(LossyWindowSettings {
            intersend_delay,
            ..current_settings
        })
    }
}
