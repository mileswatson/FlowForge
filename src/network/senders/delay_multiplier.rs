use std::fmt::Debug;

use crate::{
    core::{
        logging::Logger,
        meters::{FlowMeter, EWMA},
        rand::Rng,
    },
    network::PacketAddress,
    quantities::{Float, TimeSpan},
    simulation::{HasSubEffect, SimulatorBuilder},
};

use super::window::{
    AckReceived, LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderAddress,
    LossySenderSlot, LossyWindowBehavior, LossyWindowSender, LossyWindowSettings,
};

#[derive(Debug)]
struct Behavior {
    multiplier: Float,
    rtt: EWMA<TimeSpan>,
}

impl LossyWindowBehavior for Behavior {
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

pub struct LossyDelayMultiplierSender;

pub struct LossyDelayMultiplierSenderSlot<'sim, 'a, 'b, E>(LossySenderSlot<'sim, 'a, 'b, E>);

impl<'sim, 'a, 'b, E> LossyDelayMultiplierSenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
        + HasSubEffect<LossyInternalControllerEffect>
        + 'sim,
    'sim: 'a,
{
    #[must_use]
    pub fn address(&self) -> LossySenderAddress<'sim, E> {
        self.0.address()
    }

    pub fn set<F>(
        self,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        multiplier: Float,
        wait_for_enable: bool,
        flow_meter: F,
        rng: Rng,
        logger: impl Logger + Clone + 'a,
    ) -> LossySenderAddress<'sim, E>
    where
        F: FlowMeter + 'a,
    {
        self.0.set(
            id,
            link,
            destination,
            Box::new(move || Behavior {
                multiplier,
                rtt: EWMA::new(1. / 8.),
            }),
            wait_for_enable,
            flow_meter,
            rng,
            logger,
        )
    }
}

impl LossyDelayMultiplierSender {
    pub fn reserve_slot<'sim, 'a, 'b, E, L>(
        builder: &'b SimulatorBuilder<'sim, 'a, E>,
    ) -> LossyDelayMultiplierSenderSlot<'sim, 'a, 'b, E>
    where
        L: Logger + Clone + 'a,
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
    {
        LossyDelayMultiplierSenderSlot(LossyWindowSender::reserve_slot(builder))
    }

    pub fn insert<'sim, 'a, 'b, T, F, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        multiplier: Float,
        wait_for_enable: bool,
        flow_meter: F,
        rng: Rng,
        logger: L,
    ) -> LossySenderAddress<'sim, E>
    where
        L: Logger + Clone + 'sim,
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
        F: FlowMeter + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot::<E, L>(builder);
        slot.set(
            id,
            link,
            destination,
            multiplier,
            wait_for_enable,
            flow_meter,
            rng,
            logger,
        )
    }
}