use crate::{
    average::EWMA,
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    network::toggler::Toggle,
    simulation::{Component, ComponentId, EffectContext, HasVariant, MaybeHasVariant, Message},
    time::{Float, Time, TimeSpan},
};

use super::window::lossy_window::{
    LossyWindowBehavior, LossyWindowSender, LossyWindowSettings, Packet,
};

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
            intersend_delay: TimeSpan::new(0.),
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

impl<E, L> Component<E> for LossySender<L>
where
    E: HasVariant<Packet> + MaybeHasVariant<Toggle>,
    L: Logger,
{
    fn tick(&mut self, context: EffectContext) -> Vec<Message<E>> {
        self.0.tick(context)
    }

    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<E>> {
        self.0.receive(e, context)
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        Component::<E>::next_tick(&self.0, time)
    }
}

impl<L> Flow for LossySender<L>
where
    L: Logger,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
