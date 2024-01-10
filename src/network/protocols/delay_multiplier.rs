use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    meters::EWMA,
    network::{NetworkEffect, NetworkMessage},
    quantities::{Float, Time, TimeSpan},
    simulation::{Component, ComponentId, EffectContext},
};

use super::window::lossy_window::{LossyWindowBehavior, LossyWindowSender, LossyWindowSettings};

#[derive(Debug)]
struct Behavior {
    multiplier: Float,
    rtt: EWMA<TimeSpan>,
}

impl<L> LossyWindowBehavior<'static, L> for Behavior
where
    L: Logger,
{
    fn initial_settings(&self) -> LossyWindowSettings {
        LossyWindowSettings {
            window: 1,
            intersend_delay: TimeSpan::ZERO,
        }
    }

    fn ack_received(
        &mut self,
        current: &mut LossyWindowSettings,
        sent_time: Time,
        received_time: Time,
        logger: &mut L,
    ) {
        let rtt = self.rtt.update(received_time - sent_time);
        let intersend_delay = self.multiplier * rtt;
        log!(logger, "Updated intersend_delay to {}", intersend_delay);
        *current = LossyWindowSettings {
            intersend_delay,
            ..*current
        };
    }
}

#[derive(Debug)]
pub struct LossySender<'sim, L>(LossyWindowSender<'sim, 'static, Behavior, L>)
where
    L: Logger;

impl<'sim, L> LossySender<'sim, L>
where
    L: Logger,
{
    pub fn new(
        id: ComponentId<'sim>,
        link: ComponentId<'sim>,
        destination: ComponentId<'sim>,
        multiplier: Float,
        wait_for_enable: bool,
        logger: L,
    ) -> LossySender<'sim, L> {
        LossySender(LossyWindowSender::new(
            id,
            link,
            destination,
            Box::new(move || Behavior {
                multiplier,
                rtt: EWMA::new(1. / 8.),
            }),
            wait_for_enable,
            logger,
        ))
    }
}

impl<'sim, L> Component<'sim, NetworkEffect<'sim>> for LossySender<'sim, L>
where
    L: Logger,
{
    fn tick(&mut self, context: EffectContext<'sim>) -> Vec<NetworkMessage<'sim>> {
        self.0.tick(context)
    }

    fn receive(
        &mut self,
        e: NetworkEffect<'sim>,
        context: EffectContext<'sim>,
    ) -> Vec<NetworkMessage<'sim>> {
        self.0.receive(e, context)
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        Component::next_tick(&self.0, time)
    }
}

impl<'sim, L> Flow for LossySender<'sim, L>
where
    L: Logger,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
