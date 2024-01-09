use uom::{si::f64::Time, ConstZero};

use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    meters::EWMA,
    network::{NetworkEffect, NetworkMessage},
    simulation::{Component, ComponentId, EffectContext},
    time::{Float, Quantity, TimePoint},
};

use super::window::lossy_window::{LossyWindowBehavior, LossyWindowSender, LossyWindowSettings};

#[derive(Debug)]
struct Behavior {
    multiplier: Float,
    rtt: EWMA<Time>,
}

impl<L> LossyWindowBehavior<'static, L> for Behavior
where
    L: Logger,
{
    fn initial_settings(&self) -> LossyWindowSettings {
        LossyWindowSettings {
            window: 1,
            intersend_delay: Time::ZERO,
        }
    }

    fn ack_received(
        &mut self,
        current: &mut LossyWindowSettings,
        sent_time: TimePoint,
        received_time: TimePoint,
        logger: &mut L,
    ) {
        let rtt = self.rtt.update(received_time - sent_time);
        let intersend_delay = self.multiplier * rtt;
        log!(
            logger,
            "Updated intersend_delay to {}",
            intersend_delay.display()
        );
        *current = LossyWindowSettings {
            intersend_delay,
            ..*current
        };
    }
}

#[derive(Debug)]
pub struct LossySender<L>(LossyWindowSender<'static, Behavior, L>)
where
    L: Logger;

impl<L> LossySender<L>
where
    L: Logger,
{
    pub fn new(
        id: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        multiplier: Float,
        wait_for_enable: bool,
        logger: L,
    ) -> LossySender<L> {
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

impl<L> Component<NetworkEffect> for LossySender<L>
where
    L: Logger,
{
    fn tick(&mut self, context: EffectContext) -> Vec<NetworkMessage> {
        self.0.tick(context)
    }

    fn receive(&mut self, e: NetworkEffect, context: EffectContext) -> Vec<NetworkMessage> {
        self.0.receive(e, context)
    }

    fn next_tick(&self, time: TimePoint) -> Option<TimePoint> {
        Component::next_tick(&self.0, time)
    }
}

impl<L> Flow for LossySender<L>
where
    L: Logger,
{
    fn properties(&self, current_time: TimePoint) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
