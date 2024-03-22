use std::rc::Rc;

use crate::{
    flow::Flow,
    logging::Logger,
    meters::EWMA,
    network::PacketDestination,
    quantities::{Float, TimeSpan},
    simulation::{HasSubEffect, SimulatorBuilder},
};

use super::window::lossy_window::{
    AckReceived, LossySenderDestinations, LossySenderEffect, LossyWindowBehavior,
    LossyWindowControllerEffect, LossyWindowSender, LossyWindowSenderSlot, LossyWindowSettings,
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

pub struct LossyDelayMultiplierSenderSlot<'sim, 'a, 'b, E>(LossyWindowSenderSlot<'sim, 'a, 'b, E>);

impl<'sim, 'a, 'b, E> LossyDelayMultiplierSenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<LossySenderEffect<'sim, E>> + HasSubEffect<LossyWindowControllerEffect> + 'sim,
    'sim: 'a,
{
    #[must_use]
    pub fn destination(&self) -> LossySenderDestinations<'sim, E> {
        self.0.destination()
    }

    pub fn set(
        self,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        multiplier: Float,
        wait_for_enable: bool,
        logger: impl Logger + Clone + 'a,
    ) -> (LossySenderDestinations<'sim, E>, Rc<dyn Flow + 'a>) {
        self.0.set(
            id,
            link,
            destination,
            Box::new(move || Behavior {
                multiplier,
                rtt: EWMA::new(1. / 8.),
            }),
            wait_for_enable,
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
        E: HasSubEffect<LossySenderEffect<'sim, E>>
            + HasSubEffect<LossyWindowControllerEffect>
            + 'sim,
    {
        LossyDelayMultiplierSenderSlot(LossyWindowSender::<E, L>::reserve_slot(builder))
    }

    pub fn insert<'sim, 'a, 'b, T, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        multiplier: Float,
        wait_for_enable: bool,
        logger: L,
    ) -> (LossySenderDestinations<'sim, E>, Rc<dyn Flow + 'a>)
    where
        L: Logger + Clone + 'sim,
        E: HasSubEffect<LossySenderEffect<'sim, E>>
            + HasSubEffect<LossyWindowControllerEffect>
            + 'sim,
        'sim: 'a,
    {
        let slot = Self::reserve_slot::<E, L>(builder);
        slot.set(id, link, destination, multiplier, wait_for_enable, logger)
    }
}
